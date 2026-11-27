// A frozen reference for the bytecode catalogue. The verifier checks the
// production enum, lookup order, and accessors against every row.
macro_rules! catalogue {
    ($($byte:literal $op:ident $format:ident $term:literal;)*) => { verus! {
        spec fn record(op: Opcode) -> (u8, &'static str, Format, bool) {
            match op {
                $(Opcode::$op => ($byte, stringify!($op), Format::$format, $term),)*
            }
        }

        #[verifier::opaque]
        spec fn agrees(op: Opcode) -> bool {
            let (byte, name, format, term) = record(op);
            &&& op as u8 == byte
            &&& op.name() == name
            &&& op.format() == format
            &&& op.is_terminator() == term
            &&& (byte as int) < Opcode::listed().len()
            &&& Opcode::listed()[byte as int] == op
        }

        proof fn metadata(op: Opcode)
            ensures agrees(op),
        {
            match op { $(Opcode::$op => $op(),)* }
        }

        proof fn catalogue_size()
            ensures Opcode::listed().len() == 205,
        {
            assert(Opcode::listed().len() == 205) by(compute);
        }

        // A distinct obligation per variant keeps the solver contexts small.
        $(#[allow(non_snake_case)]
        proof fn $op()
            ensures
                agrees(Opcode::$op),
                Opcode::$op as u8 == $byte,
                Opcode::$op.name() == stringify!($op),
                Opcode::$op.format() == Format::$format,
                Opcode::$op.is_terminator() == $term,
                Opcode::listed()[$byte] == Opcode::$op,
        {
            reveal(agrees);
            catalogue_size();
            assert(Opcode::listed()[$byte] == Opcode::$op) by(compute);
        })*
    }};
}

catalogue! {
    0 Nop None false;
    1 Mov AB false;
    2 Swap AB false;
    3 LoadWide AWide false;
    4 StoreWide AWide false;
    5 LoadInt AInt false;
    6 LoadConst AConst false;
    7 LoadUndefined A false;
    8 LoadNull A false;
    9 LoadTrue A false;
    10 LoadFalse A false;
    11 LoadEmpty A false;
    12 LoadFunction A false;
    13 CheckInit AKey false;
    14 ThrowConstAssign Key true;
    15 Neg AB false;
    16 Plus AB false;
    17 ToNumeric AB false;
    18 Inc AB false;
    19 Dec AB false;
    20 BitNot AB false;
    21 Not AB false;
    22 TypeOf AB false;
    23 ToPropertyKey AB false;
    24 IsUndefinedOrNull AB false;
    25 IsUndefined AB false;
    26 IsNull AB false;
    27 TypeOfIsUndefined AB false;
    28 TypeOfIsFunction AB false;
    29 Add ABC false;
    30 Sub ABC false;
    31 Mul ABC false;
    32 Div ABC false;
    33 Mod ABC false;
    34 Pow ABC false;
    35 Shl ABC false;
    36 Sar ABC false;
    37 Shr ABC false;
    38 BitAnd ABC false;
    39 BitOr ABC false;
    40 BitXor ABC false;
    41 Eq ABC false;
    42 Neq ABC false;
    43 StrictEq ABC false;
    44 StrictNeq ABC false;
    45 Lt ABC false;
    46 Lte ABC false;
    47 Gt ABC false;
    48 Gte ABC false;
    49 InstanceOf ABC false;
    50 In ABC false;
    51 AddI ABInt false;
    52 SubI ABInt false;
    53 MulI ABInt false;
    54 BitAndI ABInt false;
    55 BitOrI ABInt false;
    56 BitXorI ABInt false;
    57 ShlI ABInt false;
    58 SarI ABInt false;
    59 ShrI ABInt false;
    60 Jmp Jump true;
    61 JTrue AJump false;
    62 JFalse AJump false;
    63 JUndefinedOrNull AJump false;
    64 Gosub AJump false;
    65 Ret A true;
    66 JLt ABJump false;
    67 JLte ABJump false;
    68 JGt ABJump false;
    69 JGte ABJump false;
    70 JNlt ABJump false;
    71 JNlte ABJump false;
    72 JNgt ABJump false;
    73 JNgte ABJump false;
    74 JEq ABJump false;
    75 JNeq ABJump false;
    76 JStrictEq ABJump false;
    77 JStrictNeq ABJump false;
    78 JLtI AShortJump false;
    79 JLteI AShortJump false;
    80 JGtI AShortJump false;
    81 JGteI AShortJump false;
    82 JNltI AShortJump false;
    83 JNlteI AShortJump false;
    84 JNgtI AShortJump false;
    85 JNgteI AShortJump false;
    86 JEqI AShortJump false;
    87 JNeqI AShortJump false;
    88 JStrictEqI AShortJump false;
    89 JStrictNeqI AShortJump false;
    90 GetField ABKey false;
    91 SetField ABKey false;
    92 DefineField ABKey false;
    93 DeleteField ABKey false;
    94 DefineGetter ABKey false;
    95 DefineSetter ABKey false;
    96 GetElem ABC false;
    97 SetElem ABC false;
    98 DefineElem ABC false;
    99 DeleteElem ABC false;
    100 GetGlobal AKey false;
    101 GetGlobalOrUndefined AKey false;
    102 SetGlobal AKey false;
    103 DefineGlobal AKey false;
    104 DeleteGlobal AKey false;
    105 DeclareGlobal Key false;
    106 DeclareGlobalLet Key false;
    107 DeclareGlobalConst Key false;
    108 CheckGlobalVar Key false;
    109 CheckGlobalLexical Key false;
    110 NewCell AB false;
    111 NewCellEmpty A false;
    112 CloseCell A false;
    113 LoadCell AB false;
    114 StoreCell AB false;
    115 CheckCellInit AKey false;
    116 LoadCaptured ACapture false;
    117 StoreCaptured ACapture false;
    118 NewSpill ACount false;
    119 LoadSlot ABInt false;
    120 StoreSlot ABInt false;
    121 NewArguments AInt false;
    122 Rest AInt false;
    123 LoadNewTarget A false;
    124 NewRegExp AConst false;
    125 ForInStart AB false;
    126 ForInNext ABJump false;
    127 AppendSpread ABC false;
    128 ThrowInternal Key true;
    129 CheckCtor A false;
    130 LoadHomeObject A false;
    131 ToObject AB false;
    132 SetProto AB false;
    133 DefineMethod ABKey false;
    134 DefineMethodComputed ABC false;
    135 SetFunctionName AB false;
    136 ThrowType Key false;
    137 ThrowReference Key true;
    138 ImportCall ABC false;
    139 IterStart ABInt false;
    140 IterNext ABC false;
    141 IterClose A false;
    142 IterCloseThrow AB false;
    143 IterCallNext ABC false;
    144 IterCall ABC false;
    145 IterCheckObject A false;
    146 IterStartAsync AB false;
    147 IterNextRaw AB false;
    148 IterResult ABC false;
    149 MakeClass ABKey false;
    150 InstallFields ABC false;
    151 InitFields AB false;
    152 RunStaticInit AB false;
    153 CallStaticBlock A false;
    154 GetSuper AB false;
    155 GetSuperValue ABInt false;
    156 PutSuperValue A false;
    157 InitDerived AB false;
    158 InitThis AB false;
    159 InitThisCell AB false;
    160 InitThisCaptured ACapture false;
    161 CheckDerivedReturn AB false;
    162 NewPrivateName AKey false;
    163 SetHomeObject AB false;
    164 NamePrivateMethod AKey false;
    165 GetPrivate ABC false;
    166 PutPrivate ABC false;
    167 DefinePrivate ABC false;
    168 PrivateIn ABC false;
    169 GetPrivateMethod ABC false;
    170 CallPrivateGetter ABC false;
    171 CallPrivateSetter ABC false;
    172 PrivateBrandIn ABC false;
    173 InitialYield None false;
    174 AsyncStart None false;
    175 Yield A false;
    176 YieldStar A false;
    177 Await A false;
    178 CopyData AB false;
    179 CopyDataExcluded ABC false;
    180 GlobalRef AKey false;
    181 GetRef ABKey false;
    182 PutRef ABKey false;
    183 NewObject ACount false;
    184 NewArray ABRange false;
    185 MakeClosure AChild false;
    186 Call Call false;
    187 CallMethod Call false;
    188 TailCall Call true;
    189 TailCallMethod Call true;
    190 New Call false;
    191 CallSpread Call false;
    192 NewSpread Call false;
    193 Return A true;
    194 ReturnUndefined None true;
    195 Throw A true;
    196 ToPropertyKeyOf ABC false;
    197 ClearCell A false;
    198 ConcatN ABRange false;
    199 JmpBack Jump true;
    200 IncGetElem AB false;
    201 NewArrayK AConstRange false;
    202 NewObjectLit ACount false;
    203 InitField ABKey false;
    204 FinishObjectLit AInt false;
}
