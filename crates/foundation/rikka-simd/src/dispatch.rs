#[macro_export]
macro_rules! dispatch {
    ($level:expr, $simd:ident => $body:expr $(,)?) => {
        match $crate::Level::token($level) {
            $crate::__private::Token::Sse2($simd) => $crate::Simd::vectorize(
                $simd,
                #[inline(always)]
                || $body,
            ),
            $crate::__private::Token::Sse42($simd) => $crate::Simd::vectorize(
                $simd,
                #[inline(always)]
                || $body,
            ),
            $crate::__private::Token::Avx2($simd) => $crate::Simd::vectorize(
                $simd,
                #[inline(always)]
                || $body,
            ),
            $crate::__private::Token::Avx512($simd) => $crate::Simd::vectorize(
                $simd,
                #[inline(always)]
                || $body,
            ),
        }
    };
}
