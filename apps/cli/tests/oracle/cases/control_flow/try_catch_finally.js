// CATCH_CASES
// case: basic primitive catch
try{throw 7}catch(e){e}
// case: optional catch binding
try{throw 8}catch{9}
// case: no-throw path skips catch
try{10}catch(e){11}
// case: primitive throw preserves type and value
try{throw 'primitive'}catch(e){typeof e+'|'+e}
// case: object throw preserves identity
try{throw Function}catch(e){typeof e+'|'+(e===Function)}
// case: VM native error is catchable
try{null.x}catch(e){e.name+'|'+e.message}
// case: native getter error is catchable
try{Function.prototype.caller}catch(e){e.name+'|'+e.message}
// case: callee throw reaches caller catch
(function(){try{(function thrownCallee(){throw 12})()}catch(e){return e}})()
// case: constructor throw reaches caller catch
(function(){var C=function ThrownConstructor(){throw 13};try{new C()}catch(e){return e}})()
// case: nearest catch wins
(function(){var s='';try{try{throw 1}catch(e){s+='i'+e}}catch(e){s+='o'+e}return s})()
// case: rethrow reaches the outer catch
(function(){try{try{throw 1}catch(e){throw e}}catch(e){return e}})()
// case: new throw from catch reaches the outer catch
(function(){try{try{throw 1}catch(e){throw 3}}catch(e){return e}})()
// CATCH_SCOPE_CASES
// case: catch binding permits same-name var
(function(){try{throw 4}catch(e){var e;return e}})()
// case: closure captures catch binding
(function(){var f;try{throw 4}catch(e){f=function(){return e}}return f()})()
// case: re-entered catch receives a fresh captured cell
(function(){var f,g,i=0;while(i<2){try{throw ++i}catch(e){if(i===1)f=function(){return e};else g=function(){return e}}}return f()+'|'+g()+'|'+(f()!==g())})()
// FINALLY_CASES
// case: normal try executes finally
(function(){var s='';try{s+='t'}finally{s+='f'}return s})()
// case: caught throw executes finally
(function(){var s='';try{s+='t';throw 'c'}catch(e){s+=e}finally{s+='f'}return s})()
// case: uncaught inner throw executes finally before outer catch
(function(){var s='';try{try{throw 1}finally{s+='f'}}catch(e){return s+'|'+e}})()
// case: return value crosses finally
(function(){var s='';var r=(function(){try{return 1}finally{s+='f'}})();return r+'|'+s})()
// case: catch inside finally preserves the outer gosub return address
(function(){var s='';var r=(function(){try{return 1}finally{try{throw 2}catch(e){s+=e}}})();return r+'|'+s})()
// case: finally return overrides pending return
(function(){try{return 1}finally{return 2}})()
// case: finally throw overrides pending throw
(function(){try{try{throw 1}finally{throw 2}}catch(e){return e}})()
// case: throw from catch still executes finally
(function(){var s='';try{try{throw 1}catch(e){s+='c';throw 2}finally{s+='f'}}catch(e){return s+'|'+e}})()
// case: unlabelled break crosses one finally
(function(){var s='';while(true){try{s+='t';break}finally{s+='f'}}return s})()
// case: unlabelled continue crosses one finally
(function(){var s='',i=0;while(i<2){i++;try{continue}finally{s+=i}}return s})()
// case: unlabelled break crosses nested finally clauses
(function(){var s='';while(true){try{try{s+='t';break}finally{s+='i'}}finally{s+='o'}}return s})()
// case: unlabelled continue crosses nested finally clauses
(function(){var s='',i=0;while(i<2){i++;try{try{if(i<2)continue;s+='b'}finally{s+='i'}}finally{s+='o'}}return s})()
// case: labelled break crosses one finally
(function(){var s='';outer:while(true){try{s+='t';break outer}finally{s+='f'}}return s})()
// case: labelled continue crosses one finally
(function(){var s='',i=0;outer:while(i<2){i++;try{continue outer}finally{s+=i}}return s})()
// case: labelled break crosses nested finally clauses
(function(){var s='';outer:while(true){try{try{s+='t';break outer}finally{s+='i'}}finally{s+='o'}}return s})()
// case: labelled continue crosses nested finally clauses
(function(){var s='',i=0;outer:while(i<2){i++;try{try{if(i<2)continue outer;s+='b'}finally{s+='i'}}finally{s+='o'}}return s})()
// case: nested throw catch and finally order
(function(){var s='';try{try{s+='t';throw 'a'}finally{s+='f'}}catch(e){s+=e}finally{s+='g'}return s})()
// case: switch discriminant remains below catch and finally state
(function(v){var s='';switch(v){case 1:try{s+='t';throw 'x'}catch(e){s+=e}finally{s+='f'}case 2:s+='2';break;default:s+='d'}return s})(1)
// COMPLETION_AND_QUIRK_CASES
// case: normal finally does not replace Script completion
try{1}finally{2}
// case: normal finally does not replace catch completion
try{throw 1}catch(e){3}finally{2}
// case: caught throw preserves QuickJS intervening captured-cell reuse
(function(){var f,g,i=0;while(i<2){i++;try{{let x=i;if(i===1)f=function(){return x};else g=function(){return x};throw 0}}catch(e){}}return f()+'|'+g()+'|'+(f===g)})()
// ABRUPT_FINALLY_CELL_REUSE_CASES
// case: return overridden by finally continue reuses the captured try lexical
(function(){var f,g,i=0;while(i<2){try{let x=++i;if(i===1)f=function(){return x};else g=function(){return x};return 9}finally{continue}}return f()+'|'+g()})()
// case: return overridden by finally break reuses the captured try lexical after outer-loop re-entry
(function(){var f,g,i=0;while(i<2){while(true){try{let x=++i;if(i===1)f=function(){return x};else g=function(){return x};return 9}finally{break}}}return f()+'|'+g()})()
// case: throw overridden by finally continue reuses the captured try lexical
(function(){var f,g,i=0;while(i<2){try{let x=++i;if(i===1)f=function(){return x};else g=function(){return x};throw 9}finally{continue}}return f()+'|'+g()})()
// case: throw overridden by finally break reuses the captured try lexical after outer-loop re-entry
(function(){var f,g,i=0;while(i<2){while(true){try{let x=++i;if(i===1)f=function(){return x};else g=function(){return x};throw 9}finally{break}}}return f()+'|'+g()})()
// case: nested finally throw then continue overrides return and re-enters the captured lexical
(function(){var f,g,i=0;while(i<2){try{try{let x=++i;if(i===1)f=function(){return x};else g=function(){return x};return 9}finally{throw 8}}finally{continue}}return f()+'|'+g()})()
// case: nested finally return then continue overrides throw and re-enters the captured lexical
(function(){var f,g,i=0;while(i<2){try{try{let x=++i;if(i===1)f=function(){return x};else g=function(){return x};throw 9}finally{return 8}}finally{continue}}return f()+'|'+g()})()
