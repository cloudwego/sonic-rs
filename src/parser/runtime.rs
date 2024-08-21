use faststr::FastStr;
use serde::de::Expected;

use super::{inner, ParseStatus};
#[cfg(target_arch = "aarch64")]
use crate::util::simd::neon;
#[cfg(target_arch = "x86_64")]
use crate::util::simd::{avx2, sse2};
use crate::{
    error::{Error, ErrorCode, Result},
    reader::{Reader, Reference},
    util::{
        arc::Arc,
        num::ParserNumber,
        simd::{v128, v256, v512},
    },
    value::{shared::Shared, visitor::JsonVisitor},
    Index, LazyValue, PointerTree,
};

pub enum Parser<R> {
    #[cfg(target_arch = "x86_64")]
    Sse2(
        inner::Parser<
            R,
            v256::Simd256i<sse2::Simd128i>,
            v256::Simd256u<sse2::Simd128u>,
            v512::Simd512u<v256::Simd256u<sse2::Simd128u>>,
        >,
    ),
    #[cfg(target_arch = "x86_64")]
    Avx2(inner::Parser<R, avx2::Simd256i, avx2::Simd256u, v512::Simd512u<avx2::Simd256u>>),
    #[cfg(target_arch = "aarch64")]
    Neon(
        inner::Parser<
            R,
            v256::Simd256i<neon::Simd128i>,
            v256::Simd256u<neon::Simd128u>,
            v512::Simd512u<v256::Simd256u<neon::Simd128u>>,
        >,
    ),
    Scalar(
        inner::Parser<
            R,
            v256::Simd256i<v128::Simd128i>,
            v256::Simd256u<v128::Simd128u>,
            v512::Simd512u<v256::Simd256u<v128::Simd128u>>,
        >,
    ),
}

macro_rules! dispatch {
    ($self:ident => $($stuff:tt)*) => {{
        cfg_if::cfg_if! {
            if #[cfg(target_arch = "x86_64")] {
                match $self {
                    Self::Avx2(val) => {
                        val.$($stuff)*
                    },
                    Self::Sse2(val) => {
                        val.$($stuff)*
                    },
                    Self::Scalar(val) => {
                        val.$($stuff)*
                    }
                }
            } else if #[cfg(target_arch = "aarch64")] {
                match $self {
                    Self::Neon(val) => {
                        val.$($stuff)*
                    },
                    Self::Scalar(val) => {
                        val.$($stuff)*
                    }
                }
            } else {
                let Self::Scalar(val) = $self;
                val.$($stuff)*
            }
        }
    }};
}

impl<'de, R> Parser<R>
where
    R: Reader<'de>,
{
    #[inline(always)]
    pub fn new(read: R) -> Self {
        cfg_if::cfg_if! {
            if #[cfg(target_arch = "x86_64")] {
                use crate::util::simd::{avx2, sse2};

                match (sse2::is_supported(), avx2::is_supported()) {
                    (false, false) => Self::Scalar(inner::Parser::new(read)),
                    (_, true) => Self::Avx2(inner::Parser::new(read)),
                    (true, false) => Self::Sse2(inner::Parser::new(read)),
                }
            } else if #[cfg(target_arch = "aarch64")] {
                use crate::util::simd::neon;

                if neon::is_supported() {
                    Self::Neon(inner::Parser::new(read))
                } else {
                    Self::Scalar(inner::Parser::new(read))
                }
            } else {
                Self::Scalar(inner::Parser::new(read))
            }
        }
    }

    #[inline(always)]
    pub(crate) fn error(&self, reason: ErrorCode) -> Error {
        dispatch!(self => error(reason))
    }

    #[inline(always)]
    pub(crate) fn fix_position(&self, err: Error) -> Error {
        dispatch!(self => fix_position(err))
    }

    #[inline(always)]
    pub(crate) fn get_from_with_iter<P: IntoIterator>(
        &mut self,
        path: P,
    ) -> Result<(&'de [u8], ParseStatus)>
    where
        P::Item: Index,
    {
        dispatch!(self => get_from_with_iter(path))
    }

    #[inline(always)]
    pub(crate) fn get_from_with_iter_checked<P: IntoIterator>(
        &mut self,
        path: P,
    ) -> Result<(&'de [u8], ParseStatus)>
    where
        P::Item: Index,
    {
        dispatch!(self => get_from_with_iter_checked(path))
    }

    #[inline(always)]
    pub(crate) fn get_many(
        &mut self,
        tree: &PointerTree,
        is_safe: bool,
    ) -> Result<Vec<LazyValue<'de>>> {
        dispatch!(self => get_many(tree, is_safe))
    }

    #[inline(always)]
    pub(crate) fn get_shared_inc_count(&mut self) -> Arc<Shared> {
        dispatch!(self => get_shared_inc_count())
    }

    #[inline(always)]
    pub(crate) fn parse_array_elem_lazy(
        &mut self,
        first: &mut bool,
        check: bool,
    ) -> Result<Option<(&'de [u8], bool)>> {
        dispatch!(self => parse_array_elem_lazy(first, check))
    }

    #[inline(always)]
    pub(crate) fn parse_array_end(&mut self) -> Result<()> {
        dispatch!(self => parse_array_end())
    }

    #[inline(always)]
    pub(crate) fn parse_entry_lazy(
        &mut self,
        strbuf: &mut Vec<u8>,
        first: &mut bool,
        check: bool,
    ) -> Result<Option<(FastStr, &'de [u8], bool)>> {
        dispatch!(self => parse_entry_lazy(strbuf, first, check))
    }

    #[inline(always)]
    pub(crate) fn parse_literal(&mut self, literal: &str) -> Result<()> {
        dispatch!(self => parse_literal(literal))
    }

    #[inline(always)]
    pub(crate) fn parse_number(&mut self, first: u8) -> Result<ParserNumber> {
        dispatch!(self => parse_number(first))
    }

    #[inline(always)]
    pub(crate) fn parse_object_clo(&mut self) -> Result<()> {
        dispatch!(self => parse_object_clo())
    }

    #[inline(always)]
    pub(crate) fn parse_str_impl<'own>(
        &mut self,
        buf: &'own mut Vec<u8>,
    ) -> Result<Reference<'de, 'own, str>> {
        dispatch!(self => parse_str_impl(buf))
    }

    #[inline(always)]
    pub(crate) fn parse_string_raw<'own>(
        &mut self,
        buf: &'own mut Vec<u8>,
    ) -> Result<Reference<'de, 'own, [u8]>> {
        dispatch!(self => parse_string_raw(buf))
    }

    #[inline(always)]
    pub(crate) fn parse_trailing(&mut self) -> Result<()> {
        dispatch!(self => parse_trailing())
    }

    #[inline(always)]
    pub(crate) fn parse_value<V>(&mut self, visitor: &mut V) -> Result<()>
    where
        V: JsonVisitor<'de>,
    {
        dispatch!(self => parse_value(visitor))
    }

    #[inline(always)]
    pub(crate) fn parse_value_without_padding<V>(&mut self, visitor: &mut V) -> Result<()>
    where
        V: JsonVisitor<'de>,
    {
        dispatch!(self => parse_value_without_padding(visitor))
    }

    #[inline(always)]
    pub(crate) fn peek_invalid_type(&mut self, peek: u8, exp: &dyn Expected) -> Error {
        dispatch!(self => peek_invalid_type(peek, exp))
    }

    #[inline(always)]
    pub(crate) fn skip_number(&mut self, first: u8) -> Result<()> {
        dispatch!(self => skip_number(first))
    }

    #[inline(always)]
    pub(crate) fn skip_one(&mut self) -> Result<(&'de [u8], ParseStatus)> {
        dispatch!(self => skip_one())
    }

    #[inline(always)]
    pub(crate) fn skip_space(&mut self) -> Option<u8> {
        dispatch!(self => skip_space())
    }

    #[inline(always)]
    pub(crate) fn skip_space_peek(&mut self) -> Option<u8> {
        dispatch!(self => skip_space_peek())
    }

    // --- FIELD GETTERS ---

    #[inline(always)]
    pub(crate) fn read(&mut self) -> &mut R {
        dispatch!(self => read())
    }
}
