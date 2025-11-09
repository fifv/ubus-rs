/// Macro for defining helpful enum-like opaque structs
#[macro_export]
macro_rules! values {
    (
        $vis:vis $name:ident ( $repr:ty ) -> $other:ty {
            $( $variant:ident = $value:literal -> $othervalue:literal ),* $(,)?
        }
    ) => {
        values!($vis $name($repr) { $( $variant = $value , )* });
        impl $name {
            pub fn other(self) -> Option<$other> {
                match self {
                    $( Self::$variant => Some($othervalue), )*
                    _ => None,
                }
            }
        }
    };
    (
        $vis:vis $name:ident ( $repr:ty ) {
            $( $variant:ident = $value:literal ),* $(,)?
        }
    ) => {
        #[repr(transparent)]
        #[derive(Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
        $vis struct $name(pub $repr);
        impl $name {
            $( pub const $variant: Self = Self($value); )*
            pub fn known(self) -> bool {
                match self {
                    $( Self::$variant => true, )*
                    _ => false,
                }
            }
            pub fn value(self) -> $repr {
                self.0
            }
        }
        impl From<$repr> for $name {
            fn from(other: $repr) -> Self {
                Self(other)
            }
        }
        impl core::fmt::Debug for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                match *self {
                    $( Self::$variant => write!(f, stringify!($variant)), )*
                    unknown => write!(f, "UNKNOWN({})", unknown.0),
                }
            }
        }
        impl core::fmt::Display for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                match *self {
                    $( Self::$variant => write!(f, stringify!($variant)), )*
                    unknown => write!(f, "UNKNOWN({})", unknown.0),
                }
            }
        }
    };
}

#[macro_export]
macro_rules! invalid_data_panic {
    ($($arg:tt)*) => (if cfg!(debug_assertions) { panic!($($arg)*); })
}

#[macro_export]
macro_rules! valid_data {
    (($left:expr) >= ($right:expr), $msg:literal) => {{
        if !(($left) >= ($right)) {
            $crate::invalid_data_panic!("Invalid Data: {} ({:?} < {:?})", $msg, $left, $right);
            return Err(Error::InvalidData($msg));
        }
    }};
    (($left:expr) == ($right:expr), $msg:literal) => {{
        if !(($left) == ($right)) {
            $crate::invalid_data_panic!("Invalid Data: {} ({:?} != {:?})", $msg, $left, $right);
            return Err(Error::InvalidData($msg));
        }
    }};
    ($thing:expr, $msg:literal) => {{
        if !($thing) {
            $crate::invalid_data_panic!("Invalid Data: {}", $msg);
            return Err(UbusError::InvalidData($msg));
        }
    }};
}
