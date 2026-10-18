//! A sketch link to geometry of another component can keep the exact placement it reads
//! (`placement: {source: [occurrence ids], author: [occurrence ids]}`): the occurrence path of
//! the source (`A:2`, not whichever copy of the part comes first) and the placement of the
//! sketch's own component it was made in. A link can also be frozen (`frozen: true`): its
//! geometry stays as it is until it is resumed. Older designs need no transformation: a link
//! without `placement` reads its source where the sketch's component sees it, as before, and one
//! without `frozen` is live. The last release refuses a design with this format number and asks
//! for an update, rather than resolving a placed link against the wrong copy of a part.
