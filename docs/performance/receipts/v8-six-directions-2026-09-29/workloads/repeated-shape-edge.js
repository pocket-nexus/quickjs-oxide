// Keep both shapes alive so each own-property append reuses one weak edge.
var emptyShapeOwner = Object.create(null);
var targetShapeOwner = Object.create(null);
targetShapeOwner.x = 0;
var transitionChecksum = 0;
for (var transitionIndex = 0; transitionIndex < 100000; transitionIndex++) {
    var transitionObject = Object.create(null);
    transitionObject.x = transitionIndex;
    transitionChecksum += transitionObject.x;
}
print(transitionChecksum);
