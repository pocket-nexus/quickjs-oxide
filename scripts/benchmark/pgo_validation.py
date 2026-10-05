"""Validate the immutable training chain before accepting a PGO build."""
import json
from pathlib import Path

from run import digest


def require(condition, message):
    if not condition:
        raise ValueError(message)


def verify_file(item):
    require(digest(item["path"]) == item["sha256"], f"PGO artifact changed: {item['path']}")


def verify_training(receipt_path, profdata, source):
    receipt = json.loads(Path(receipt_path).read_text())
    require(receipt.get("schema") == "oxide-pgo-training-v1", "verified PGO training receipt required")
    require(receipt["source_commit"] == source["commit"]["stdout"]
            and receipt["source_tree"] == source["tree"]["stdout"], "PGO training source differs")
    require(receipt.get("held_out") == "original-v8-v7", "V8 v7 must remain held out of training")
    require(receipt.get("samples") and all(row["status"] == "ok" and row["exit_code"] == 0
            and not row["timed_out"] for row in receipt["samples"]), "PGO training did not complete correctly")
    require(receipt.get("profraw"), "PGO training contains no raw profiles")
    produced = []
    workloads = {(row["case"], row["size"]): row for row in receipt["workloads"]}
    require(len(workloads) == len(receipt["samples"]), "PGO training workload/sample count differs")
    for sample in receipt["samples"]:
        for stream in ("stdout", "stderr"):
            verify_file({"path": sample[stream], "sha256": sample[stream + "_sha256"]})
        workload = workloads[(sample["case"], sample["size"])]
        require(Path(sample["stdout"]).read_bytes() == workload["expected"].encode(), "training output differs from expected")
        require(sample.get("profraw"), "training process produced no raw profile")
        for item in sample["profraw"]:
            verify_file(item)
            require(Path(item["path"]).stat().st_size > 0, "empty training process raw profile")
            produced.append((item["path"], item["sha256"]))
    require(len(produced) == len(set(produced)) and sorted(produced)
            == sorted((row["path"], row["sha256"]) for row in receipt["profraw"]),
            "per-process raw profiles do not match merged inventory")
    require(Path(receipt["profdata"]["path"]).resolve() == Path(profdata).resolve(), "PGO merged data path differs")
    for item in receipt["artifacts"] + receipt["profraw"] + [receipt["profdata"]]:
        verify_file(item)
    generated_item = receipt["instrumented_build_receipt"]
    verify_file(generated_item)
    generated = json.loads(Path(generated_item["path"]).read_text())
    verify_file(receipt["instrumented_binary"])
    require(receipt["instrumented_binary"]["sha256"] == generated["binary_sha256"], "training binary differs from build receipt")
    require(generated.get("mode") == "profile-generate" and generated.get("features") == []
            and generated.get("exit_code") == 0, "training used an invalid instrumented build")
    require(generated["commit"] == receipt["source_commit"]
            and generated["source"]["tree"]["stdout"] == receipt["source_tree"]
            and generated["source"]["status"]["stdout"] == "", "instrumented training source differs")
    require(generated["rustc"] == receipt["rustc"] and generated["cargo"] == receipt["cargo"],
            "instrumented training compiler differs")
    require(generated["release_profile"]["qjs_rustc_invocation"]["codegen"].get("profile-generate")
            == generated["optimization"]["data"], "observed training instrumentation differs")
    return receipt


def require_build_technique(baseline, candidate):
    """Only predeclared PGO use may differ; the engine source must be identical."""
    left, right = baseline["build"], candidate["build"]
    require(left.get("mode") == "plain" and left.get("optimization", {}).get("kind", "none") == "none",
            "build-technique baseline must be uninstrumented release")
    require(right.get("mode") == "profile-use" and right.get("optimization", {}).get("kind") == "profile-use",
            "build-technique candidate must explicitly be profile-use")
    require(left["commit"] == right["commit"] and left["source"]["tree"]["stdout"] == right["source"]["tree"]["stdout"],
            "build-technique source differs")
    for name in ("rustc", "cargo", "target", "features", "cargo_toml_sha256", "cargo_lock_sha256"):
        require(left[name] == right[name], f"build-technique {name} differs")
    for name in ("manifest", "cargo_config_sha256"):
        require(left["release_profile"][name] == right["release_profile"][name], f"build-technique release {name} differs")
    for build in (left, right):
        require(build["target"]["requested_triple"] is not None, "PGO comparison requires an explicit target")
        require(build["features"] == [] and build.get("exit_code") == 0 and build["source"]["status"]["stdout"] == "",
                "PGO comparison requires successful clean uninstrumented builds")
        require(build.get("schema") == "oxide-build-v2" and build["source"]["commit"]["stdout"] == build["commit"],
                "PGO comparison requires matching source and v2 build receipts")
    optimization = right["optimization"]
    evidence = optimization["training_receipt"]
    verify_file(evidence)
    receipt = verify_training(evidence["path"], optimization["data"], right["source"])
    require(receipt["profdata"]["sha256"] == optimization["data_sha256"], "PGO used data differs from merged data")
    require(receipt["rustc"] == right["rustc"] and receipt["cargo"] == right["cargo"], "PGO training compiler differs")
    generated = json.loads(Path(receipt["instrumented_build_receipt"]["path"]).read_text())
    for name in ("rustc", "cargo", "target", "features", "cargo_toml_sha256", "cargo_lock_sha256"):
        require(generated[name] == left[name], f"generate and none {name} differ")
    for name in ("manifest", "cargo_config_sha256"):
        require(generated["release_profile"][name] == left["release_profile"][name], f"generate and none release {name} differ")
    generated_env = dict(generated["release_profile"]["environment_overrides"])
    generated_flags = generated_env.get("CARGO_ENCODED_RUSTFLAGS", "").split("\x1f")
    generate_flag = f"-Cprofile-generate={generated['optimization']['data']}"
    require(generated_flags.count(generate_flag) == 1, "effective generate flag missing or repeated")
    generated_flags.remove(generate_flag)
    generated_env["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(generated_flags)
    require(generated_env == left["release_profile"]["environment_overrides"], "generate environment differs beyond profile-generate")
    generated_codegen = dict(generated["release_profile"]["qjs_rustc_invocation"]["codegen"])
    require(generated_codegen.pop("profile-generate", None) == generated["optimization"]["data"]
            and generated_codegen == left["release_profile"]["qjs_rustc_invocation"]["codegen"],
            "generate effective codegen differs beyond profile-generate")
    a, b = (dict(build["release_profile"]["environment_overrides"]) for build in (left, right))
    encoded = b.get("CARGO_ENCODED_RUSTFLAGS", "").split("\x1f")
    expected = f"-Cprofile-use={optimization['data']}"
    require(encoded.count(expected) == 1, "effective PGO flag missing or repeated")
    encoded.remove(expected)
    b["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(encoded)
    require(a == b, "build-technique environment differs beyond declared profile-use")
    for field in ("qjs_rustc_invocation",):
        a, b = (dict(build["release_profile"][field]["codegen"]) for build in (left, right))
        require(b.pop("profile-use", None) == optimization["data"], "observed profile-use differs")
        require("profile-generate" not in a and "profile-generate" not in b and a == b,
                "effective codegen differs beyond declared profile-use")
