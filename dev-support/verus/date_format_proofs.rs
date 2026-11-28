verus! {
// Connect the receiver's field-selection policy in the parent module with
// the actual family and part chosen in the formatter.
fn method_selection(method: DateStringMethod) {
    let local = method.uses_local_time();
    let format = method.format();
    let part = method.part();
    let date = part.has_date();
    let time = part.has_time();
    proof {
        let expected = match method {
            DateStringMethod::UtcString => (false, Format::Utc, true, true),
            DateStringMethod::IsoString => (false, Format::Iso, true, true),
            DateStringMethod::String => (true, Format::Local, true, true),
            DateStringMethod::DateString => (true, Format::Local, true, false),
            DateStringMethod::TimeString => (true, Format::Local, false, true),
            DateStringMethod::LocaleString => (true, Format::Locale, true, true),
            DateStringMethod::LocaleDateString => (true, Format::Locale, true, false),
            DateStringMethod::LocaleTimeString => (true, Format::Locale, false, true),
        };
        assert((local, format, date, time) == expected);
    }
}
}
