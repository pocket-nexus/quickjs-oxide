macro_rules! verify_opcode_list {
    ($(#[$doc:meta])* pub const $name:ident: &'static [Opcode] = &[$($op:path,)*];) => {
        verus_impl! {
            pub closed spec fn listed() -> Seq<Opcode> {
                let table = [$($op,)*];
                table@
            }

            $(#[$doc])*
            pub exec const $name: &'static [Opcode]
                ensures Self::$name@ == Self::listed(), Self::$name@.len() <= 256,
            {
                &[$($op,)*]
            }
        }
    };
}
