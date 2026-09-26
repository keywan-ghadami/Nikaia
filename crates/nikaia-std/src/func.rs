//! **A function value that is kept**: in a field, a result, a `let`, or a
//! parameter the callee stores (Part II 12.3's handlers, Part I 5.3).
//!
//! A parameter the callee only runs is a closure argument and never reaches
//! here. One that is kept needs a type a struct can hold, so it is one shared
//! closure: `Kept<dyn Fn(A) -> R>`, cloned by a count, printed as `<fn>`, and
//! called as the closure it holds. A function type that may pause keeps a
//! closure returning a [`Boxed`] future.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// One kept closure.
pub struct Kept<F: ?Sized>(pub Arc<F>);

/// What a kept function that may pause hands back.
pub type Boxed<T> = Pin<Box<dyn Future<Output = T>>>;

/// The same, where tasks run on threads and the future may cross one.
pub type SendBoxed<T> = Pin<Box<dyn Future<Output = T> + Send>>;

impl<F: ?Sized> Clone for Kept<F> {
    fn clone(&self) -> Self {
        Kept(self.0.clone())
    }
}

impl<F: ?Sized> std::fmt::Debug for Kept<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<fn>")
    }
}

impl<F: ?Sized> std::ops::Deref for Kept<F> {
    type Target = F;
    fn deref(&self) -> &F {
        &self.0
    }
}

macro_rules! arity {
    ($($a:ident),*) => {
        impl<C, R, $($a),*> From<C> for Kept<dyn Fn($($a),*) -> R>
        where
            C: Fn($($a),*) -> R + 'static,
        {
            fn from(c: C) -> Self {
                Kept(Arc::new(c))
            }
        }
    };
}

arity!();
arity!(A1);
arity!(A1, A2);
arity!(A1, A2, A3);
arity!(A1, A2, A3, A4);
arity!(A1, A2, A3, A4, A5);
arity!(A1, A2, A3, A4, A5, A6);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kept_closure_is_called_cloned_and_printed() {
        let n = 3;
        let add: Kept<dyn Fn(i64) -> i64> = Kept::from(move |x| x + n);
        let again = add.clone();
        assert_eq!(add(1) + again(2), 9);
        assert_eq!(format!("{add:?}"), "<fn>");
    }
}
