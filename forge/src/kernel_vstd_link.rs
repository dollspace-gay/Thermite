//! Erased, `no_std` Rust metadata stub for the pinned Verus `vstd` slice model.
//!
//! The semantic authority is the separately imported, digest-bound `vstd.vir`.
//! This crate contains only the matching Rust/Verus item skeleton needed by
//! rustc while checking and linking an erased kernel artifact. Keep the item
//! and impl order aligned with the pinned `vstd::{seq,view,slice}` sources: the
//! imported Verus metadata identifies external impls by their definition path.

#![no_std]
#![allow(unused_imports)]

use verus_builtin::*;
use verus_builtin_macros::*;

pub mod seq {
    use super::*;
    use core::marker;

    verus! {
        #[verifier::external_body]
        #[verifier::ext_equal]
        #[verifier::accept_recursive_types(A)]
        pub tracked struct Seq<A> {
            dummy: marker::PhantomData<A>,
        }

        impl<A> Seq<A> {
            pub uninterp spec fn new(len: nat, f: impl Fn(int) -> A) -> Seq<A>;
            pub uninterp spec fn len(self) -> nat;
            pub uninterp spec fn index(self, i: int) -> A;

            #[verifier::inline]
            pub open spec fn spec_index(self, i: int) -> A {
                self.index(i)
            }
        }
    }
}

pub mod view {
    use super::*;

    verus! {
        pub trait View {
            type V;
            spec fn view(&self) -> Self::V;
        }

        pub trait DeepView {
            type V;
            spec fn deep_view(&self) -> Self::V;
        }
    }
}

pub mod slice {
    use super::*;
    use crate::seq::Seq;
    use crate::view::{DeepView, View};

    verus! {
        impl<T> View for [T] {
            type V = Seq<T>;
            uninterp spec fn view(&self) -> Seq<T>;
        }

        impl<T: DeepView> DeepView for [T] {
            type V = Seq<T::V>;

            open spec fn deep_view(&self) -> Seq<T::V> {
                let v = self.view();
                Seq::new(v.len(), |i: int| v[i].deep_view())
            }
        }

        pub trait SliceAdditionalSpecFns<T>: View<V = Seq<T>> {
            spec fn spec_index(&self, i: int) -> T;
        }

        impl<T> SliceAdditionalSpecFns<T> for [T] {
            #[verifier::inline]
            open spec fn spec_index(&self, i: int) -> T {
                self.view().index(i)
            }
        }
    }
}

pub mod prelude {
    pub use crate::seq::Seq;
    pub use crate::slice::SliceAdditionalSpecFns;
    pub use crate::view::{DeepView, View};
    pub use verus_builtin::*;
    pub use verus_builtin_macros::*;
}
