# 固定版本 Unicode 生成表

保存上游固定版本对应的 Unicode 数据及生成说明。正常构建直接读取这些文件，不执行生成器。

## 文件与子目录

- [unicode/](unicode/README.md)：子模块职责与文件说明。

## 补充实现说明

# Generated production data

`unicode/` contains the four checksum-pinned Rust Unicode tables. The
handwritten algorithms in `src/source/unicode/` include these files in private
table modules; generated files add no public API.

Regenerate with the corresponding `scripts/unicode/generate-unicode-*-tables`
command and the pinned QuickJS source, following the source and license
information in each header. Generators default to `unicode/`. Ordinary builds
consume the checked-in tables without running generators or a reference engine.

To verify an update, generate into a temporary output file and compare it with
the checked-in file; run the Unicode library tests and the normalization
fingerprint check. Test262 metadata belongs in `dev-support/test262/generated/`.
