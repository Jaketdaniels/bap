use crate::NumericError;

macro_rules! checked_integer {
    ($name:ident, $inner:ty) => {
        #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
        pub struct $name($inner);

        impl $name {
            pub const fn new(value: $inner) -> Self {
                Self(value)
            }

            pub const fn get(self) -> $inner {
                self.0
            }

            pub fn checked_add(self, rhs: Self) -> Result<Self, NumericError> {
                self.0
                    .checked_add(rhs.0)
                    .map(Self)
                    .ok_or(NumericError::Overflow)
            }

            pub fn checked_sub(self, rhs: Self) -> Result<Self, NumericError> {
                self.0
                    .checked_sub(rhs.0)
                    .map(Self)
                    .ok_or(NumericError::Overflow)
            }

            pub fn checked_mul(self, rhs: Self) -> Result<Self, NumericError> {
                self.0
                    .checked_mul(rhs.0)
                    .map(Self)
                    .ok_or(NumericError::Overflow)
            }

            pub fn checked_div(self, rhs: Self) -> Result<Self, NumericError> {
                if rhs.0 == 0 {
                    return Err(NumericError::DivisionByZero);
                }
                self.0
                    .checked_div(rhs.0)
                    .map(Self)
                    .ok_or(NumericError::Overflow)
            }

            pub fn checked_neg(self) -> Result<Self, NumericError> {
                self.0.checked_neg().map(Self).ok_or(NumericError::Overflow)
            }
        }
    };
}

checked_integer!(CheckedI32, i32);
checked_integer!(CheckedI64, i64);
