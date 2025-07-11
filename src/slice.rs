#[cfg(not(feature = "std"))]
use crate::prelude::*;
#[cfg(feature = "nightly")]
use core::alloc::Allocator;
use core::fmt::{Debug, Formatter, Result};
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::mem;
use core::ops::{Index, IndexMut};
use core::ptr::NonNull;

use crate::array::Array;
use crate::dim::{Const, Dim, Dyn};
use crate::expr::{Apply, Expression, FromExpression, IntoExpression};
use crate::expr::{AxisExpr, AxisExprMut, Iter, Lanes, LanesMut, Map, Zip};
use crate::index::{Axis, Cols, DimIndex, Permutation, Resize, Rows, SliceIndex, Split, ViewIndex};
use crate::layout::{Dense, Layout, Strided};
use crate::mapping::Mapping;
use crate::raw_slice::RawSlice;
use crate::shape::{ConstShape, DynRank, IntoShape, Rank, Shape};
use crate::tensor::Tensor;
use crate::traits::{IntoCloned, Owned};
use crate::view::{View, ViewMut};

/// Multidimensional array slice.
pub struct Slice<T, S: Shape = DynRank, L: Layout = Dense> {
    phantom: PhantomData<(T, S, L)>,
}

/// Multidimensional array slice with dynamically-sized dimensions.
pub type DSlice<T, const N: usize, L = Dense> = Slice<T, Rank<N>, L>;

impl<T, S: Shape, L: Layout> Slice<T, S, L> {
    /// Returns a mutable pointer to the array buffer.
    pub fn as_mut_ptr(&mut self) -> *mut T {
        if mem::size_of::<L::Mapping<S>>() > 0 {
            RawSlice::from_mut_slice(self).as_mut_ptr()
        } else {
            self as *mut Self as *mut T
        }
    }

    /// Returns a raw pointer to the array buffer.
    pub fn as_ptr(&self) -> *const T {
        if mem::size_of::<L::Mapping<S>>() > 0 {
            RawSlice::from_slice(self).as_ptr()
        } else {
            self as *const Self as *const T
        }
    }

    /// Assigns an expression to the array slice with broadcasting, cloning elements if needed.
    ///
    /// # Panics
    ///
    /// Panics if the expression cannot be broadcast to the shape of the array slice.
    pub fn assign<I: IntoExpression<Item: IntoCloned<T>>>(&mut self, expr: I) {
        self.expr_mut().zip(expr).for_each(|(x, y)| y.clone_to(x));
    }

    /// Returns an array view after indexing the first dimension.
    ///
    /// # Panics
    ///
    /// Panics if the index is out of bounds, or if the rank is not at least 1.
    pub fn at(&self, index: usize) -> View<T, S::Tail, L> {
        self.axis_at(Const::<0>, index)
    }

    /// Returns a mutable array view after indexing the first dimension.
    ///
    /// # Panics
    ///
    /// Panics if the index is out of bounds, or if the rank is not at least 1.
    pub fn at_mut(&mut self, index: usize) -> ViewMut<T, S::Tail, L> {
        self.axis_at_mut(Const::<0>, index)
    }

    /// Returns an array view after indexing the specified dimension.
    ///
    /// If the dimension to be indexed is know at compile time, the resulting array shape
    /// will maintain constant-sized dimensions. Furthermore, if it is the first dimension
    /// the resulting array view has the same layout as the input.
    ///
    /// # Panics
    ///
    /// Panics if the dimension or the index is out of bounds.
    pub fn axis_at<A: Axis>(&self, axis: A, index: usize) -> View<T, A::Remove<S>, Split<A, S, L>> {
        unsafe { View::axis_at(self.as_ptr(), self.mapping(), axis, index) }
    }

    /// Returns a mutable array view after indexing the specified dimension.
    ///
    /// If the dimension to be indexed is know at compile time, the resulting array shape
    /// will maintain constant-sized dimensions. Furthermore, if it is the first dimension
    /// the resulting array view has the same layout as the input.
    ///
    /// # Panics
    ///
    /// Panics if the dimension or the index is out of bounds.
    pub fn axis_at_mut<A: Axis>(
        &mut self,
        axis: A,
        index: usize,
    ) -> ViewMut<T, A::Remove<S>, Split<A, S, L>> {
        unsafe { ViewMut::axis_at(self.as_mut_ptr(), self.mapping(), axis, index) }
    }

    /// Returns an expression that gives array views iterating over the specified dimension.
    ///
    /// If the dimension to be iterated over is know at compile time, the resulting array
    /// shape will maintain constant-sized dimensions. Furthermore, if it is the first
    /// dimension the resulting array views have the same layout as the input.
    ///
    /// # Panics
    ///
    /// Panics if the dimension is out of bounds.
    pub fn axis_expr<A: Axis>(&self, axis: A) -> AxisExpr<T, S, L, A> {
        AxisExpr::new(self, axis)
    }

    /// Returns a mutable expression that gives array views iterating over the specified dimension.
    ///
    /// If the dimension to be iterated over is know at compile time, the resulting array
    /// shape will maintain constant-sized dimensions. Furthermore, if it is the first
    /// dimension the resulting array views have the same layout as the input.
    ///
    /// # Panics
    ///
    /// Panics if the dimension is out of bounds.
    pub fn axis_expr_mut<A: Axis>(&mut self, axis: A) -> AxisExprMut<T, S, L, A> {
        AxisExprMut::new(self, axis)
    }

    /// Returns an array view for the specified column.
    ///
    /// # Panics
    ///
    /// Panics if the rank is not equal to 2, or if the index is out of bounds.
    pub fn col(&self, index: usize) -> View<T, (S::Head,), Strided> {
        let shape = self.shape().with_dims(<(_, <S::Tail as Shape>::Head)>::from_dims);

        self.reshape(shape).into_view(.., index)
    }

    /// Returns a mutable array view for the specified column.
    ///
    /// # Panics
    ///
    /// Panics if the rank is not equal to 2, or if the index is out of bounds.
    pub fn col_mut(&mut self, index: usize) -> ViewMut<T, (S::Head,), Strided> {
        let shape = self.shape().with_dims(<(_, <S::Tail as Shape>::Head)>::from_dims);

        self.reshape_mut(shape).into_view(.., index)
    }

    /// Returns an expression that gives column views iterating over the other dimensions.
    ///
    /// # Panics
    ///
    /// Panics if the rank is not at least 2.
    pub fn cols(&self) -> Lanes<T, S, L, Cols> {
        self.lanes(Cols)
    }

    /// Returns a mutable expression that gives column views iterating over the other dimensions.
    ///
    /// # Panics
    ///
    /// Panics if the rank is not at least 2.
    pub fn cols_mut(&mut self) -> LanesMut<T, S, L, Cols> {
        self.lanes_mut(Cols)
    }

    /// Returns `true` if the array slice contains an element with the given value.
    pub fn contains(&self, x: &T) -> bool
    where
        T: PartialEq,
    {
        contains(self, x)
    }

    /// Returns an array view for the given diagonal of the array slice,
    /// where `index` > 0 is above and `index` < 0 is below the main diagonal.
    ///
    /// # Panics
    ///
    /// Panics if the rank is not equal to 2, or if the absolute index is larger
    /// than the number of columns or rows.
    pub fn diag(&self, index: isize) -> View<T, (Dyn,), Strided> {
        let shape = self.shape().with_dims(<(S::Head, <S::Tail as Shape>::Head)>::from_dims);

        self.reshape(shape).into_diag(index)
    }

    /// Returns a mutable array view for the given diagonal of the array slice,
    /// where `index` > 0 is above and `index` < 0 is below the main diagonal.
    ///
    /// # Panics
    ///
    /// Panics if the rank is not equal to 2, or if the absolute index is larger
    /// than the number of columns or rows.
    pub fn diag_mut(&mut self, index: isize) -> ViewMut<T, (Dyn,), Strided> {
        let shape = self.shape().with_dims(<(S::Head, <S::Tail as Shape>::Head)>::from_dims);

        self.reshape_mut(shape).into_diag(index)
    }

    /// Returns the number of elements in the specified dimension.
    ///
    /// # Panics
    ///
    /// Panics if the dimension is out of bounds.
    pub fn dim(&self, index: usize) -> usize {
        self.mapping().dim(index)
    }

    /// Returns an expression over the array slice.
    pub fn expr(&self) -> View<T, S, L> {
        unsafe { View::new_unchecked(self.as_ptr(), self.mapping().clone()) }
    }

    /// Returns a mutable expression over the array slice.
    pub fn expr_mut(&mut self) -> ViewMut<T, S, L> {
        unsafe { ViewMut::new_unchecked(self.as_mut_ptr(), self.mapping().clone()) }
    }

    /// Fills the array slice with elements by cloning `value`.
    pub fn fill(&mut self, value: T)
    where
        T: Clone,
    {
        self.expr_mut().for_each(|x| x.clone_from(&value));
    }

    /// Fills the array slice with elements returned by calling a closure repeatedly.
    pub fn fill_with<F: FnMut() -> T>(&mut self, mut f: F) {
        self.expr_mut().for_each(|x| *x = f());
    }

    /// Returns a one-dimensional array view of the array slice.
    ///
    /// # Panics
    ///
    /// Panics if the array layout is not uniformly strided.
    pub fn flatten(&self) -> View<T, (Dyn,), L> {
        self.reshape([self.len()])
    }

    /// Returns a mutable one-dimensional array view over the array slice.
    ///
    /// # Panics
    ///
    /// Panics if the array layout is not uniformly strided.
    pub fn flatten_mut(&mut self) -> ViewMut<T, (Dyn,), L> {
        self.reshape_mut([self.len()])
    }

    /// Returns a reference to an element or a subslice, without doing bounds checking.
    ///
    /// # Safety
    ///
    /// The index must be within bounds of the array slice.
    pub unsafe fn get_unchecked<I: SliceIndex<T, S, L>>(&self, index: I) -> &I::Output {
        unsafe { index.get_unchecked(self) }
    }

    /// Returns a mutable reference to an element or a subslice, without doing bounds checking.
    ///
    /// # Safety
    ///
    /// The index must be within bounds of the array slice.
    pub unsafe fn get_unchecked_mut<I: SliceIndex<T, S, L>>(&mut self, index: I) -> &mut I::Output {
        unsafe { index.get_unchecked_mut(self) }
    }

    /// Returns `true` if the array strides are consistent with contiguous memory layout.
    pub fn is_contiguous(&self) -> bool {
        self.mapping().is_contiguous()
    }

    /// Returns `true` if the array contains no elements.
    pub fn is_empty(&self) -> bool {
        self.mapping().is_empty()
    }

    /// Returns an iterator over the array slice.
    pub fn iter(&self) -> Iter<View<'_, T, S, L>> {
        self.expr().into_iter()
    }

    /// Returns a mutable iterator over the array slice.
    pub fn iter_mut(&mut self) -> Iter<ViewMut<'_, T, S, L>> {
        self.expr_mut().into_iter()
    }

    /// Returns an expression that gives array views over the specified dimension,
    /// iterating over the other dimensions.
    ///
    /// If the dimension to give array views over is know at compile time, the resulting
    /// shape will maintain a constant-sized dimension. Furthermore, if it is the last
    /// dimension the resulting array views have the same layout as the input.
    ///
    /// # Panics
    ///
    /// Panics if the dimension is out of bounds.
    pub fn lanes<A: Axis>(&self, axis: A) -> Lanes<T, S, L, A> {
        Lanes::new(self, axis)
    }

    /// Returns a mutable expression that gives array views over the specified dimension,
    /// iterating over the other dimensions.
    ///
    /// If the dimension to give array views over is know at compile time, the resulting
    /// shape will maintain a constant-sized dimension. Furthermore, if it is the last
    /// dimension the resulting array views have the same layout as the input.
    ///
    /// # Panics
    ///
    /// Panics if the dimension is out of bounds.
    pub fn lanes_mut<A: Axis>(&mut self, axis: A) -> LanesMut<T, S, L, A> {
        LanesMut::new(self, axis)
    }

    /// Returns the number of elements in the array.
    pub fn len(&self) -> usize {
        self.mapping().len()
    }

    /// Returns the array layout mapping.
    pub fn mapping(&self) -> &L::Mapping<S> {
        if mem::size_of::<L::Mapping<S>>() > 0 {
            RawSlice::from_slice(self).mapping()
        } else {
            unsafe { &*NonNull::dangling().as_ptr() }
        }
    }

    /// Returns an expression that gives array views iterating over the first dimension.
    ///
    /// Iterating over the first dimension results in array views with the same layout
    /// as the input.
    ///
    /// # Panics
    ///
    /// Panics if the rank is not at least 1.
    pub fn outer_expr(&self) -> AxisExpr<T, S, L, Const<0>> {
        self.axis_expr(Const::<0>)
    }

    /// Returns a mutable expression that gives array views iterating over the first dimension.
    ///
    /// Iterating over the first dimension results in array views with the same layout
    /// as the input.
    ///
    /// # Panics
    ///
    /// Panics if the rank is not at least 1.
    pub fn outer_expr_mut(&mut self) -> AxisExprMut<T, S, L, Const<0>> {
        self.axis_expr_mut(Const::<0>)
    }

    /// Returns an array view with the dimensions permuted.
    ///
    /// If the permutation is an identity permutation and known at compile time, the
    /// resulting array view has the same layout as the input. For example, permuting
    /// with `(Const::<0>, Const::<1>)` will maintain the layout while permuting with
    /// `[0, 1]` gives strided layout.
    ///
    /// # Panics
    ///
    /// Panics if the permutation is not valid.
    pub fn permute<I: IntoShape<IntoShape: Permutation>>(
        &self,
        perm: I,
    ) -> View<T, <I::IntoShape as Permutation>::Shape<S>, <I::IntoShape as Permutation>::Layout<L>>
    {
        let mapping = perm.into_dims(|dims| Mapping::permute(self.mapping(), dims));

        unsafe { View::new_unchecked(self.as_ptr(), mapping) }
    }

    /// Returns a mutable array view with the dimensions permuted.
    ///
    /// If the permutation is an identity permutation and known at compile time, the
    /// resulting array view has the same layout as the input. For example, permuting
    /// with `(Const::<0>, Const::<1>)` will maintain the layout while permuting with
    /// `[0, 1]` gives strided layout.
    ///
    /// # Panics
    ///
    /// Panics if the permutation is not valid.
    pub fn permute_mut<I: IntoShape<IntoShape: Permutation>>(
        &mut self,
        perm: I,
    ) -> ViewMut<T, <I::IntoShape as Permutation>::Shape<S>, <I::IntoShape as Permutation>::Layout<L>>
    {
        let mapping = perm.into_dims(|dims| Mapping::permute(self.mapping(), dims));

        unsafe { ViewMut::new_unchecked(self.as_mut_ptr(), mapping) }
    }

    /// Returns the array rank, i.e. the number of dimensions.
    pub fn rank(&self) -> usize {
        self.mapping().rank()
    }

    /// Returns a remapped array view of the array slice.
    ///
    /// # Panics
    ///
    /// Panics if the memory layout is not compatible with the new array layout.
    pub fn remap<R: Shape, K: Layout>(&self) -> View<T, R, K> {
        let mapping = Mapping::remap(self.mapping());

        unsafe { View::new_unchecked(self.as_ptr(), mapping) }
    }

    /// Returns a mutable remapped array view of the array slice.
    ///
    /// # Panics
    ///
    /// Panics if the memory layout is not compatible with the new array layout.
    pub fn remap_mut<R: Shape, K: Layout>(&mut self) -> ViewMut<T, R, K> {
        let mapping = Mapping::remap(self.mapping());

        unsafe { ViewMut::new_unchecked(self.as_mut_ptr(), mapping) }
    }

    /// Returns a reordered array view of the array slice.
    ///
    /// This method is deprecated, use `transpose` instead.
    #[deprecated]
    pub fn reorder(&self) -> View<T, S::Reverse, <S::Tail as Shape>::Layout<L>> {
        let mapping = Mapping::transpose(self.mapping());

        unsafe { View::new_unchecked(self.as_ptr(), mapping) }
    }

    /// Returns a mutable reordered array view of the array slice.
    ///
    /// This method is deprecated, use `transpose_mut` instead.
    #[deprecated]
    pub fn reorder_mut(&mut self) -> ViewMut<T, S::Reverse, <S::Tail as Shape>::Layout<L>> {
        let mapping = Mapping::transpose(self.mapping());

        unsafe { ViewMut::new_unchecked(self.as_mut_ptr(), mapping) }
    }

    /// Returns a reshaped array view of the array slice.
    ///
    /// At most one dimension can have dynamic size `usize::MAX`, and is then inferred
    /// from the other dimensions and the array length.
    ///
    /// # Examples
    ///
    /// ```
    /// use mdarray::view;
    ///
    /// let v = view![[1, 2, 3], [4, 5, 6]];
    ///
    /// assert_eq!(v.reshape([!0, 2]), view![[1, 2], [3, 4], [5, 6]]);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the array length is changed, or if the memory layout is not compatible.
    pub fn reshape<I: IntoShape>(&self, shape: I) -> View<T, I::IntoShape, L> {
        let mapping = self.mapping().reshape(shape.into_shape());

        unsafe { View::new_unchecked(self.as_ptr(), mapping) }
    }

    /// Returns a mutable reshaped array view of the array slice.
    ///
    /// At most one dimension can have dynamic size `usize::MAX`, and is then inferred
    /// from the other dimensions and the array length.
    ///
    /// See the `reshape` method above for examples.
    ///
    /// # Panics
    ///
    /// Panics if the array length is changed, or if the memory layout is not compatible.
    pub fn reshape_mut<I: IntoShape>(&mut self, shape: I) -> ViewMut<T, I::IntoShape, L> {
        let mapping = self.mapping().reshape(shape.into_shape());

        unsafe { ViewMut::new_unchecked(self.as_mut_ptr(), mapping) }
    }

    /// Returns an array view for the specified row.
    ///
    /// # Panics
    ///
    /// Panics if the rank is not equal to 2, or if the index is out of bounds.
    pub fn row(&self, index: usize) -> View<T, (<S::Tail as Shape>::Head,), L> {
        let shape = self.shape().with_dims(<(S::Head, _)>::from_dims);

        self.reshape(shape).into_view(index, ..)
    }

    /// Returns a mutable array view for the specified row.
    ///
    /// # Panics
    ///
    /// Panics if the rank is not equal to 2, or if the index is out of bounds.
    pub fn row_mut(&mut self, index: usize) -> ViewMut<T, (<S::Tail as Shape>::Head,), L> {
        let shape = self.shape().with_dims(<(S::Head, _)>::from_dims);

        self.reshape_mut(shape).into_view(index, ..)
    }

    /// Returns an expression that gives row views iterating over the other dimensions.
    ///
    /// # Panics
    ///
    /// Panics if the rank is not at least 1.
    pub fn rows(&self) -> Lanes<T, S, L, Rows> {
        self.lanes(Rows)
    }

    /// Returns a mutable expression that gives row views iterating over the other dimensions.
    ///
    /// # Panics
    ///
    /// Panics if the rank is not at least 1.
    pub fn rows_mut(&mut self) -> LanesMut<T, S, L, Rows> {
        self.lanes_mut(Rows)
    }

    /// Returns the array shape.
    pub fn shape(&self) -> &S {
        self.mapping().shape()
    }

    /// Divides an array slice into two at an index along the first dimension.
    ///
    /// # Panics
    ///
    /// Panics if the split point is larger than the number of elements in that dimension,
    /// or if the rank is not at least 1.
    pub fn split_at(
        &self,
        mid: usize,
    ) -> (View<T, Resize<Const<0>, S>, L>, View<T, Resize<Const<0>, S>, L>) {
        self.split_axis_at(Const::<0>, mid)
    }

    /// Divides a mutable array slice into two at an index along the first dimension.
    ///
    /// # Panics
    ///
    /// Panics if the split point is larger than the number of elements in that dimension,
    /// or if the rank is not at least 1.
    pub fn split_at_mut(
        &mut self,
        mid: usize,
    ) -> (ViewMut<T, Resize<Const<0>, S>, L>, ViewMut<T, Resize<Const<0>, S>, L>) {
        self.split_axis_at_mut(Const::<0>, mid)
    }

    /// Divides an array slice into two at an index along the specified dimension.
    ///
    /// If the dimension to be divided is know at compile time, the resulting array
    /// shape will maintain constant-sized dimensions. Furthermore, if it is the first
    /// dimension the resulting array views have the same layout as the input.
    ///
    /// # Panics
    ///
    /// Panics if the split point is larger than the number of elements in that dimension,
    /// or if the dimension is out of bounds.
    pub fn split_axis_at<A: Axis>(
        &self,
        axis: A,
        mid: usize,
    ) -> (View<T, Resize<A, S>, Split<A, S, L>>, View<T, Resize<A, S>, Split<A, S, L>>) {
        unsafe { View::split_axis_at(self.as_ptr(), self.mapping(), axis, mid) }
    }

    /// Divides a mutable array slice into two at an index along the specified dimension.
    ///
    /// If the dimension to be divided is know at compile time, the resulting array
    /// shape will maintain constant-sized dimensions. Furthermore, if it is the first
    /// dimension the resulting array views have the same layout as the input.
    ///
    /// # Panics
    ///
    /// Panics if the split point is larger than the number of elements in that dimension,
    /// or if the dimension is out of bounds.
    pub fn split_axis_at_mut<A: Axis>(
        &mut self,
        axis: A,
        mid: usize,
    ) -> (ViewMut<T, Resize<A, S>, Split<A, S, L>>, ViewMut<T, Resize<A, S>, Split<A, S, L>>) {
        unsafe { ViewMut::split_axis_at(self.as_mut_ptr(), self.mapping(), axis, mid) }
    }

    /// Returns the distance between elements in the specified dimension.
    ///
    /// # Panics
    ///
    /// Panics if the dimension is out of bounds.
    pub fn stride(&self, index: usize) -> isize {
        self.mapping().stride(index)
    }

    /// Copies the array slice into a new array.
    pub fn to_array(&self) -> Array<T, S>
    where
        T: Clone,
        S: ConstShape,
    {
        Array::from(self)
    }

    /// Copies the array slice into a new array.
    pub fn to_tensor(&self) -> Tensor<T, S>
    where
        T: Clone,
    {
        Tensor::from(self)
    }

    /// Copies the array slice into a new array with the specified allocator.
    #[cfg(feature = "nightly")]
    pub fn to_tensor_in<A: Allocator>(&self, alloc: A) -> Tensor<T, S, A>
    where
        T: Clone,
    {
        Tensor::from_expr_in(self.expr().cloned(), alloc)
    }

    /// Copies the array slice into a new vector.
    pub fn to_vec(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.to_tensor().into_vec()
    }

    /// Copies the array slice into a new vector with the specified allocator.
    #[cfg(feature = "nightly")]
    pub fn to_vec_in<A: Allocator>(&self, alloc: A) -> Vec<T, A>
    where
        T: Clone,
    {
        self.to_tensor_in(alloc).into_vec()
    }

    /// Returns a transposed array view of the array slice, where the dimensions
    /// are reversed.
    pub fn transpose(&self) -> View<T, S::Reverse, <S::Tail as Shape>::Layout<L>> {
        let mapping = Mapping::transpose(self.mapping());

        unsafe { View::new_unchecked(self.as_ptr(), mapping) }
    }

    /// Returns a mutable transposed array view of the array slice, where the dimensions
    /// are reversed.
    pub fn transpose_mut(&mut self) -> ViewMut<T, S::Reverse, <S::Tail as Shape>::Layout<L>> {
        let mapping = Mapping::transpose(self.mapping());

        unsafe { ViewMut::new_unchecked(self.as_mut_ptr(), mapping) }
    }
}

impl<T, L: Layout> Slice<T, DynRank, L> {
    /// Returns the number of elements in each dimension.
    pub fn dims(&self) -> &[usize] {
        self.mapping().dims()
    }
}

impl<T, S: Shape> Slice<T, S, Strided> {
    /// Returns the distance between elements in each dimension.
    pub fn strides(&self) -> &[isize] {
        self.mapping().strides()
    }
}

macro_rules! impl_view {
    (($($xyz:tt),+), ($($abc:tt),+), ($($idx:tt),+)) => {
        impl<T, $($xyz: Dim,)+ L: Layout> Slice<T, ($($xyz,)+), L> {
            /// Copies the specified subarray into a new array.
            ///
            /// # Panics
            ///
            /// Panics if the subarray is out of bounds.
            pub fn array<$($abc: DimIndex),+>(
                &self,
                $($idx: $abc),+
            ) -> Array<T, <($($abc,)+) as ViewIndex>::Shape<($($xyz,)+)>>
            where
                T: Clone,
                ($($abc,)+): ViewIndex<Shape<($($xyz,)+)>: ConstShape>,
            {
                self.view($($idx),+).to_array()
            }

            /// Copies the specified subarray into a new array.
            ///
            /// # Panics
            ///
            /// Panics if the subarray is out of bounds.
            pub fn tensor<$($abc: DimIndex),+>(
                &self,
                $($idx: $abc),+
            ) -> Tensor<T, <($($abc,)+) as ViewIndex>::Shape<($($xyz,)+)>>
            where
                T: Clone,
            {
                self.view($($idx),+).to_tensor()
            }

            /// Returns an array view for the specified subarray.
            ///
            /// # Panics
            ///
            /// Panics if the subarray is out of bounds.
            pub fn view<$($abc: DimIndex),+>(
                &self,
                $($idx: $abc),+
            ) -> View<
                T,
                <($($abc,)+) as ViewIndex>::Shape<($($xyz,)+)>,
                <($($abc,)+) as ViewIndex>::Layout<L>,
            > {
                self.expr().into_view($($idx),+)
            }

            /// Returns a mutable array view for the specified subarray.
            ///
            /// # Panics
            ///
            /// Panics if the subarray is out of bounds.
            pub fn view_mut<$($abc: DimIndex),+>(
                &mut self,
                $($idx: $abc),+,
            ) -> ViewMut<
                T,
                <($($abc,)+) as ViewIndex>::Shape<($($xyz,)+)>,
                <($($abc,)+) as ViewIndex>::Layout<L>,
            > {
                self.expr_mut().into_view($($idx),+)
            }
        }
    };
}

impl_view!((X), (A), (a));
impl_view!((X, Y), (A, B), (a, b));
impl_view!((X, Y, Z), (A, B, C), (a, b, c));
impl_view!((X, Y, Z, W), (A, B, C, D), (a, b, c, d));
impl_view!((X, Y, Z, W, U), (A, B, C, D, E), (a, b, c, d, e));
impl_view!((X, Y, Z, W, U, V), (A, B, C, D, E, F), (a, b, c, d, e, f));

impl<'a, T, U, S: Shape, L: Layout> Apply<U> for &'a Slice<T, S, L> {
    type Output<F: FnMut(&'a T) -> U> = Map<Self::IntoExpr, F>;
    type ZippedWith<I: IntoExpression, F: FnMut((&'a T, I::Item)) -> U> =
        Map<Zip<Self::IntoExpr, I::IntoExpr>, F>;

    fn apply<F: FnMut(&'a T) -> U>(self, f: F) -> Self::Output<F> {
        self.expr().map(f)
    }

    fn zip_with<I: IntoExpression, F>(self, expr: I, f: F) -> Self::ZippedWith<I, F>
    where
        F: FnMut((&'a T, I::Item)) -> U,
    {
        self.expr().zip(expr).map(f)
    }
}

impl<'a, T, U, S: Shape, L: Layout> Apply<U> for &'a mut Slice<T, S, L> {
    type Output<F: FnMut(&'a mut T) -> U> = Map<Self::IntoExpr, F>;
    type ZippedWith<I: IntoExpression, F: FnMut((&'a mut T, I::Item)) -> U> =
        Map<Zip<Self::IntoExpr, I::IntoExpr>, F>;

    fn apply<F: FnMut(&'a mut T) -> U>(self, f: F) -> Self::Output<F> {
        self.expr_mut().map(f)
    }

    fn zip_with<I: IntoExpression, F>(self, expr: I, f: F) -> Self::ZippedWith<I, F>
    where
        F: FnMut((&'a mut T, I::Item)) -> U,
    {
        self.expr_mut().zip(expr).map(f)
    }
}

impl<T, S: Shape, L: Layout> AsMut<Self> for Slice<T, S, L> {
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl<T, D: Dim> AsMut<[T]> for Slice<T, (D,)> {
    fn as_mut(&mut self) -> &mut [T] {
        self.expr_mut().into()
    }
}

impl<T, S: Shape, L: Layout> AsRef<Self> for Slice<T, S, L> {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<T, D: Dim> AsRef<[T]> for Slice<T, (D,)> {
    fn as_ref(&self) -> &[T] {
        self.expr().into()
    }
}

macro_rules! impl_as_mut_ref {
    (($($xyz:tt),+), $array:tt) => {
        impl<T, $(const $xyz: usize),+> AsMut<$array> for Slice<T, ($(Const<$xyz>,)+)> {
            fn as_mut(&mut self) -> &mut $array {
                unsafe { &mut *(self as *mut Self as *mut $array) }
            }
        }

        impl<T, $(const $xyz: usize),+> AsRef<$array> for Slice<T, ($(Const<$xyz>,)+)> {
            fn as_ref(&self) -> &$array {
                unsafe { &*(self as *const Self as *const $array) }
            }
        }
    };
}

impl_as_mut_ref!((X), [T; X]);
impl_as_mut_ref!((X, Y), [[T; Y]; X]);
impl_as_mut_ref!((X, Y, Z), [[[T; Z]; Y]; X]);
impl_as_mut_ref!((X, Y, Z, W), [[[[T; W]; Z]; Y]; X]);
impl_as_mut_ref!((X, Y, Z, W, U), [[[[[T; U]; W]; Z]; Y]; X]);
impl_as_mut_ref!((X, Y, Z, W, U, V), [[[[[[T; V]; U]; W]; Z]; Y]; X]);

impl<T: Debug, S: Shape, L: Layout> Debug for Slice<T, S, L> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        if self.rank() == 0 {
            self[[]].fmt(f)
        } else {
            f.debug_list().entries(self.outer_expr()).finish()
        }
    }
}

impl<T: Hash, S: Shape, L: Layout> Hash for Slice<T, S, L> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for i in 0..self.rank() {
            #[cfg(not(feature = "nightly"))]
            state.write_usize(self.dim(i));
            #[cfg(feature = "nightly")]
            state.write_length_prefix(self.dim(i));
        }

        self.expr().for_each(|x| x.hash(state));
    }
}

impl<T, S: Shape, L: Layout, I: SliceIndex<T, S, L>> Index<I> for Slice<T, S, L> {
    type Output = I::Output;

    fn index(&self, index: I) -> &I::Output {
        index.index(self)
    }
}

impl<T, S: Shape, L: Layout, I: SliceIndex<T, S, L>> IndexMut<I> for Slice<T, S, L> {
    fn index_mut(&mut self, index: I) -> &mut I::Output {
        index.index_mut(self)
    }
}

impl<'a, T, S: Shape, L: Layout> IntoExpression for &'a Slice<T, S, L> {
    type Shape = S;
    type IntoExpr = View<'a, T, S, L>;

    fn into_expr(self) -> Self::IntoExpr {
        self.expr()
    }
}

impl<'a, T, S: Shape, L: Layout> IntoExpression for &'a mut Slice<T, S, L> {
    type Shape = S;
    type IntoExpr = ViewMut<'a, T, S, L>;

    fn into_expr(self) -> Self::IntoExpr {
        self.expr_mut()
    }
}

impl<'a, T, S: Shape, L: Layout> IntoIterator for &'a Slice<T, S, L> {
    type Item = &'a T;
    type IntoIter = Iter<View<'a, T, S, L>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T, S: Shape, L: Layout> IntoIterator for &'a mut Slice<T, S, L> {
    type Item = &'a mut T;
    type IntoIter = Iter<ViewMut<'a, T, S, L>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T: Clone, S: Shape> ToOwned for Slice<T, S> {
    type Owned = S::Owned<T>;

    fn to_owned(&self) -> Self::Owned {
        FromExpression::from_expr(self.into_expr().cloned())
    }

    fn clone_into(&self, target: &mut Self::Owned) {
        target.clone_from_slice(self);
    }
}

fn contains<T: PartialEq, S: Shape, L: Layout>(this: &Slice<T, S, L>, value: &T) -> bool {
    if L::IS_DENSE {
        this.remap::<S, _>()[..].contains(value)
    } else if this.rank() < 2 {
        this.iter().any(|x| x == value)
    } else {
        this.outer_expr().into_iter().any(|x| x.contains(value))
    }
}
