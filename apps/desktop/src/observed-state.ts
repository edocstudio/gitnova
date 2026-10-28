// Reuse equal, freshly observed DTOs so idle polling does not rebuild the graph,
// file lists and diff dock. This never skips a native read or authorizes a write.
export function reuseObserved<T>(previous:T|undefined,current:T):T{
 return previous!==undefined && JSON.stringify(previous)===JSON.stringify(current) ? previous : current;
}
