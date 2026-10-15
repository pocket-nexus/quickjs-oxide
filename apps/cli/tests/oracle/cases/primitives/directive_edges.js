// mode: print
// case: continued use-strict string is not a directive
'use strict' + ''; 010
// case: QuickJS directive ASI excludes logical not
'use strict'
!0; 010
// case: QuickJS directive ASI excludes void
'use strict'
void 0; 010
