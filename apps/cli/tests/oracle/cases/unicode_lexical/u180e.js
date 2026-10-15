function __units(value){
    value=String(value);
    var output="",index=0;
    while(index<value.length){
        var unit=value.charCodeAt(index).toString(16);
        while(unit.length<4)unit="0"+unit;
        if(index)output+=",";
        output+=unit;
        index++;
    }
    return output;
}
function __number(value){
    if(value!==value)return "NaN";
    if(value===0&&1/value===-Infinity)return "-0";
    return String(value);
}
// RAW_TOKEN_CASES
// case: U+180E is not a token separator
var᠎foo;
// SOURCE_CONTENT_CASES
// case: comments accept U+180E while string template and RegExp literals preserve it
(function(){
        var string="᠎",template=`᠎`,regexp=/᠎/,single=0,multi=0;
        //᠎ U+180E in a single-line comment
        single++;
        /*᠎ U+180E in a multi-line comment */
        multi++;
        return [
            __units(string),__units(template),__units(regexp.source),
            regexp.test(string),single,multi
        ].join("|");
    })()
// NUMBER_CASES
// case: Number rejects and prefix parsers stop at leading or trailing U+180E
(function(){
        return [
            __number(Number("\u180e")),
            __number(Number("\u180e1")),
            __number(Number("1\u180e")),
            __number(parseInt("\u180e12",10)),
            __number(parseInt("12\u180e",10)),
            __number(parseFloat("\u180e1.5")),
            __number(parseFloat("1.5\u180e"))
        ].join("|");
    })()
// TRIM_CASES
// case: trim scans stop at U+180E independently from either side
(function(){
        var both=" \t\u180e \tX\t \u180e\t ",
            leading=" \t\u180e \tX",
            trailing="X\t \u180e\t ";
        return [
            __units(both.trim()),
            __units(leading.trimStart()),__units(leading.trimEnd()),
            __units(trailing.trimStart()),__units(trailing.trimEnd()),
            __units("\u180e".trim())
        ].join("|");
    })()
// FINAL_SIGMA_CASES
// case: U+180E is Case_Ignorable for ordinary and locale-named lowercase
(function(){
        var values=[
            "A\u180e\u03a3","A\u180e\u03a3B",
            "A\u03a3\u180e","A\u03a3\u180eB",
            "A\u180e\u03a3\u180e","A\u180e\u03a3\u180eB"
        ],output=[],index=0;
        while(index<values.length){
            output.push(__units(values[index].toLowerCase()));
            output.push(__units(values[index].toLocaleLowerCase()));
            index++;
        }
        return output.join("|");
    })()
// REGEXP_CASES
// case: dot and non-whitespace match U+180E under legacy Unicode and dotAll modes
(function(){
        var flags=["","u","s","su"],output=[],index=0,value="\u180e";
        while(index<flags.length){
            var flag=flags[index++],dot=new RegExp("^.$",flag),
                space=new RegExp("^\\s$",flag),nonspace=new RegExp("^\\S$",flag);
            output.push([
                flag||"empty",dot.test(value),space.test(value),nonspace.test(value),
                dot.dotAll,dot.unicode
            ].join(":"));
        }
        return output.join("|");
    })()
