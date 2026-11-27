#[kani::proof]
#[kani::unwind(20)]
fn invalid_date_result() {
    use DateStringMethod::*;
    let kind = match kani::any::<u8>() {
        0 => String, 1 => DateString, 2 => TimeString, 3 => UtcString,
        4 => IsoString, 5 => LocaleString, 6 => LocaleDateString, _ => LocaleTimeString,
    };
    match format_date_string(None, kind) {
        Ok(text) => {
            assert!(kind != IsoString);
            assert_eq!(text.as_bytes(), b"Invalid Date");
        }
        Err(InvalidIsoDate) => assert!(kind == IsoString),
    }
}
