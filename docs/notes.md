# Observations

Writing simple data coeffects is easy to the point of being trivial.
The anymap just allows you to wish for the data to appear, and the
compiler is satisfied that it _could_ be there. Up to you to make sure
it is by injecting that coeffect.

Writing effects that manage state is not as easy.  Interestingly, they
all will probably follow the form of the Db state mutation wrapper.

Actually, since that is generic it might be possible and correct to
use it to call anything that needs a mutable reference.

The weird hard thing about that model for state mutation is that it's
not very checkable. The state mutation is cleverly hidden inside of a
closure, and thus can just be invoked, but also is totally
uninspectable from the outside.  This is the opposite of what re-frame
allows where the new app state is just whatever you return. That gets
swapped in atomically, and persistent data structures mean that you
get the efficiency of shared structure and non-interference without
copying.

To stay true to the good things about re-frame, we would be forced to
copy the state into the coeffects, and then we could remove it and
modify it, and then insert it into the effects to be swapped back into
the canonical state.

This has the advantage of not requiring the Rc, RefCell song and
dance, and it takes us back to the inspectable nature of re-frame
events.  At the cost of some copying.
