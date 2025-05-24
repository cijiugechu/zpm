use colored::{Color, Colorize};
use serde::{Deserialize, Deserializer};

use crate::{impl_serialization_traits_no_serde, FromFileString, ToFileString, ToHumanString};

const STRING_COLOR: Color
    = Color::TrueColor { r: 50, g: 170, b: 80 };

const NUMBER_COLOR: Color
    = Color::TrueColor { r: 255, g: 215, b: 0 };

const BOOLEAN_COLOR: Color
    = Color::TrueColor { r: 250, g: 160, b: 35 };

const NULL_COLOR: Color
    = Color::TrueColor { r: 160, g: 80, b: 180 };

const CODE_COLOR: Color
    = Color::TrueColor { r: 135, g: 175, b: 255 };

const PATH_COLOR: Color
    = Color::TrueColor { r: 215, g: 95, b: 215 };

const URL_COLOR: Color
    = Color::TrueColor { r: 215, g: 95, b: 215 };

pub enum DataType {
    String,
    Number,
    Boolean,
    Null,
    Code,
    Path,
    Url,
}

impl DataType {
    pub fn color(&self) -> Color {
        match self {
            DataType::String => STRING_COLOR,
            DataType::Number => NUMBER_COLOR,
            DataType::Boolean => BOOLEAN_COLOR,
            DataType::Null => NULL_COLOR,
            DataType::Code => CODE_COLOR,
            DataType::Path => PATH_COLOR,
            DataType::Url => URL_COLOR,
        }
    }

    pub fn colorize(&self, value: &str) -> String {
        value.color(self.color()).to_string()
    }
}

#[derive(Debug)]
pub struct JsonPath(Vec<String>);

impl JsonPath {
    pub fn new(path: Vec<String>) -> Self {
        Self(path)
    }

    pub fn segments(&self) -> &[String] {
        &self.0
    }
}

impl FromFileString for JsonPath {
    type Error = std::convert::Infallible;

    fn from_file_string(s: &str) -> Result<Self, Self::Error> {
        unimplemented!()
    }
}

impl ToFileString for JsonPath {
    fn to_file_string(&self) -> String {
        let mut result
            = String::new();

        for (i, segment) in self.0.iter().enumerate() {
            if i > 0 {
                result.push_str(".");
            }

            let serialized
                = sonic_rs::to_string(segment).unwrap();

            if serialized.len() == segment.len() + 2 {
                result.push_str(&segment);
            } else {
                result.push_str("[");
                result.push_str(&serialized);
                result.push_str("]");
            }
        }

        result
    }
}

impl ToHumanString for JsonPath {
    fn to_print_string(&self) -> String {
        let mut result
            = String::new();

        for (i, segment) in self.0.iter().enumerate() {
            if i > 0 {
                result.push_str(&DataType::Code.colorize("."));
            }

            let serialized
                = sonic_rs::to_string(segment).unwrap();

            if serialized.len() == segment.len() + 2 {
                result.push_str(&DataType::Code.colorize(segment));
            } else {
                result.push_str(&DataType::Code.colorize("["));
                result.push_str(&DataType::String.colorize(&serialized));
                result.push_str(&DataType::Code.colorize("]"));
            }
        }

        result
    }
}

impl_serialization_traits_no_serde!(JsonPath);

impl<'de> Deserialize<'de> for JsonPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        Ok(JsonPath::new(Vec::<String>::deserialize(deserializer)?))
    }
}
