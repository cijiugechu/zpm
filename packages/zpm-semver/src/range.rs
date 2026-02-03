use std::borrow::Borrow;

use rkyv::Archive;
use zpm_utils::{
    impl_file_string_from_str, impl_file_string_serialization, DataType, FromFileString,
    ToFileString, ToHumanString,
};

use crate::Error;

use super::{extract, Version};

#[cfg(test)]
#[path = "./range.test.rs"]
mod range_tests;

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[rkyv(derive(PartialEq, Eq, Hash, PartialOrd, Ord))]
pub enum RangeKind {
    Caret,
    Tilde,
    Exact,
}

impl FromFileString for RangeKind {
    type Error = Error;

    fn from_file_string(raw: &str) -> Result<Self, Self::Error> {
        match raw {
            "^" | "caret" => Ok(RangeKind::Caret),
            "~" | "tilde" => Ok(RangeKind::Tilde),
            "=" | "exact" | "*" | "" => Ok(RangeKind::Exact),
            _ => Err(Error::InvalidRange(raw.to_string())),
        }
    }
}

impl ToFileString for RangeKind {
    fn to_file_string(&self) -> String {
        match self {
            RangeKind::Caret => "^".to_string(),
            RangeKind::Tilde => "~".to_string(),
            RangeKind::Exact => "*".to_string(),
        }
    }
}

impl ToHumanString for RangeKind {
    fn to_print_string(&self) -> String {
        match self {
            RangeKind::Caret => "^".to_string(),
            RangeKind::Tilde => "~".to_string(),
            RangeKind::Exact => "=".to_string(),
        }
    }
}

impl_file_string_from_str!(RangeKind);
impl_file_string_serialization!(RangeKind);

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[rkyv(derive(PartialEq, Eq, Hash, PartialOrd, Ord))]
pub enum TokenType {
    LParen,
    RParen,
    SAnd,
    And,
    Or,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[rkyv(derive(PartialEq, Eq, Hash, PartialOrd, Ord))]
pub enum OperatorType {
    Equal,
    LessThan,
    GreaterThan,
    LessThanOrEqual,
    GreaterThanOrEqual,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[rkyv(derive(PartialEq, Eq, Hash, PartialOrd, Ord))]
pub enum Token {
    Syntax(TokenType),
    Operation(OperatorType, Version),
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[rkyv(derive(PartialEq, Eq, Hash, PartialOrd, Ord))]
pub enum Predicate {
    Including(Version),
    Excluding(Version),
}

impl Predicate {
    fn version(&self) -> &Version {
        match self {
            Predicate::Including(version) | Predicate::Excluding(version) => version,
        }
    }

    fn is_including(&self) -> bool {
        matches!(self, Predicate::Including(_))
    }

    fn is_excluding(&self) -> bool {
        matches!(self, Predicate::Excluding(_))
    }
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[rkyv(derive(PartialEq, Eq, Hash, PartialOrd, Ord))]
pub enum Bound {
    Unbounded,
    Lower(Predicate),
    Upper(Predicate),
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[rkyv(derive(PartialEq, Eq, Hash, PartialOrd, Ord))]
pub struct BoundSet {
    lower: Bound,
    upper: Bound,
}

impl BoundSet {
    fn new(lower: Bound, upper: Bound) -> Option<Self> {
        if lower_gt_upper(&lower, &upper) {
            return None;
        }

        Some(Self { lower, upper })
    }

    fn at_least(p: Predicate) -> Option<Self> {
        BoundSet::new(Bound::Lower(p), Bound::Unbounded)
    }

    fn at_most(p: Predicate) -> Option<Self> {
        BoundSet::new(Bound::Unbounded, Bound::Upper(p))
    }

    fn exact(version: Version) -> Option<Self> {
        BoundSet::new(
            Bound::Lower(Predicate::Including(version.clone())),
            Bound::Upper(Predicate::Including(version)),
        )
    }

    fn satisfies(&self, version: &Version) -> bool {
        let lower_ok = match &self.lower {
            Bound::Unbounded => true,
            Bound::Lower(Predicate::Including(lower)) => lower <= version,
            Bound::Lower(Predicate::Excluding(lower)) => lower < version,
            Bound::Upper(_) => unreachable!("lower bound should not be upper"),
        };

        if !lower_ok {
            return false;
        }

        let upper_ok = match &self.upper {
            Bound::Unbounded => true,
            Bound::Upper(Predicate::Including(upper)) => version <= upper,
            Bound::Upper(Predicate::Excluding(upper)) => version < upper,
            Bound::Lower(_) => unreachable!("upper bound should not be lower"),
        };

        lower_ok && upper_ok
    }

    fn intersect(&self, other: &Self) -> Option<Self> {
        let lower = max_lower(&self.lower, &other.lower);
        let upper = min_upper(&self.upper, &other.upper);
        BoundSet::new(lower, upper)
    }

    fn min_candidate(&self) -> Option<Version> {
        match &self.lower {
            Bound::Unbounded => None,
            Bound::Lower(Predicate::Including(version)) => Some(version.clone()),
            Bound::Lower(Predicate::Excluding(version)) => Some(version.next_immediate_spec()),
            Bound::Upper(_) => None,
        }
    }

    fn is_unbounded(&self) -> bool {
        matches!(self.lower, Bound::Unbounded) && matches!(self.upper, Bound::Unbounded)
    }
}

fn lower_gt_upper(lower: &Bound, upper: &Bound) -> bool {
    match (lower, upper) {
        (Bound::Unbounded, _) => false,
        (_, Bound::Unbounded) => false,
        (Bound::Lower(lower_pred), Bound::Upper(upper_pred)) => {
            let lower_version = lower_pred.version();
            let upper_version = upper_pred.version();

            match lower_version.cmp(upper_version) {
                std::cmp::Ordering::Less => false,
                std::cmp::Ordering::Greater => true,
                std::cmp::Ordering::Equal => {
                    !(lower_pred.is_including() && upper_pred.is_including())
                }
            }
        }
        _ => false,
    }
}

fn compare_predicate_lower(a: &Predicate, b: &Predicate) -> std::cmp::Ordering {
    match a.version().cmp(b.version()) {
        std::cmp::Ordering::Equal => {
            let a_order = if a.is_including() { 0u8 } else { 1u8 };
            let b_order = if b.is_including() { 0u8 } else { 1u8 };
            a_order.cmp(&b_order)
        }
        other => other,
    }
}

fn compare_predicate_upper(a: &Predicate, b: &Predicate) -> std::cmp::Ordering {
    match a.version().cmp(b.version()) {
        std::cmp::Ordering::Equal => {
            let a_order = if a.is_excluding() { 0u8 } else { 1u8 };
            let b_order = if b.is_excluding() { 0u8 } else { 1u8 };
            a_order.cmp(&b_order)
        }
        other => other,
    }
}

fn compare_lower_bound(a: &Bound, b: &Bound) -> std::cmp::Ordering {
    match (a, b) {
        (Bound::Unbounded, Bound::Unbounded) => std::cmp::Ordering::Equal,
        (Bound::Unbounded, _) => std::cmp::Ordering::Less,
        (_, Bound::Unbounded) => std::cmp::Ordering::Greater,
        (Bound::Lower(a_pred), Bound::Lower(b_pred)) => compare_predicate_lower(a_pred, b_pred),
        _ => std::cmp::Ordering::Equal,
    }
}

fn compare_upper_bound(a: &Bound, b: &Bound) -> std::cmp::Ordering {
    match (a, b) {
        (Bound::Unbounded, Bound::Unbounded) => std::cmp::Ordering::Equal,
        (Bound::Unbounded, _) => std::cmp::Ordering::Greater,
        (_, Bound::Unbounded) => std::cmp::Ordering::Less,
        (Bound::Upper(a_pred), Bound::Upper(b_pred)) => compare_predicate_upper(a_pred, b_pred),
        _ => std::cmp::Ordering::Equal,
    }
}

fn max_lower(a: &Bound, b: &Bound) -> Bound {
    if compare_lower_bound(a, b) == std::cmp::Ordering::Less {
        b.clone()
    } else {
        a.clone()
    }
}

fn min_upper(a: &Bound, b: &Bound) -> Bound {
    if compare_upper_bound(a, b) == std::cmp::Ordering::Less {
        a.clone()
    } else {
        b.clone()
    }
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[rkyv(derive(PartialEq, Eq, Hash, PartialOrd, Ord))]
pub struct Range {
    pub source: String,

    sets: Vec<BoundSet>,
    exact_version: Option<Version>,
}

impl Range {
    fn tokenize<P: AsRef<str>>(str: P) -> Option<Vec<Token>> {
        extract::extract_tokens(&mut str.as_ref().chars().peekable())
    }

    fn build_sets(tokens: &[Token]) -> Option<Vec<BoundSet>> {
        let mut n = 0usize;
        let sets = Range::build_sets_from(tokens, &mut n)?;
        Some(sets)
    }

    fn build_sets_from(tokens: &[Token], n: &mut usize) -> Option<Vec<BoundSet>> {
        let token = tokens.get(*n)?;
        *n += 1;

        match token {
            Token::Syntax(TokenType::SAnd) | Token::Syntax(TokenType::And) => {
                let left = Range::build_sets_from(tokens, n)?;
                let right = Range::build_sets_from(tokens, n)?;

                let mut out = Vec::new();
                for l in &left {
                    for r in &right {
                        if let Some(intersection) = l.intersect(r) {
                            out.push(intersection);
                        }
                    }
                }
                Some(out)
            }

            Token::Syntax(TokenType::Or) => {
                let mut left = Range::build_sets_from(tokens, n)?;
                let mut right = Range::build_sets_from(tokens, n)?;
                left.append(&mut right);
                Some(left)
            }

            Token::Operation(op, version) => {
                Some(vec![token_to_boundset(*op, version.clone())])
            }

            _ => None,
        }
    }

    pub fn any() -> Range {
        Range {
            source: "*".to_string(),
            sets: vec![
                BoundSet::new(Bound::Unbounded, Bound::Unbounded)
                    .expect("Unbounded range should be valid"),
            ],
            exact_version: None,
        }
    }

    pub fn lte(version: Version) -> Range {
        Range {
            source: format!("<={}", version.to_file_string()),
            sets: vec![
                BoundSet::at_most(Predicate::Including(version))
                    .expect("Upper bound should be valid"),
            ],
            exact_version: None,
        }
    }

    pub fn caret(version: Version) -> Range {
        let upper_bound = match (version.major, version.minor) {
            (0, 0) => version.next_patch_rc(),
            (0, _) => version.next_minor_rc(),
            _ => version.next_major_rc(),
        };

        Range {
            source: format!("^{}", version.to_file_string()),
            sets: vec![
                BoundSet::new(
                    Bound::Lower(Predicate::Including(version)),
                    Bound::Upper(Predicate::Excluding(upper_bound)),
                )
                .expect("Caret bound should be valid"),
            ],
            exact_version: None,
        }
    }

    pub fn tilde(version: Version) -> Range {
        let upper_bound = version.next_minor_rc();

        Range {
            source: format!("~{}", version.to_file_string()),
            sets: vec![
                BoundSet::new(
                    Bound::Lower(Predicate::Including(version)),
                    Bound::Upper(Predicate::Excluding(upper_bound)),
                )
                .expect("Tilde bound should be valid"),
            ],
            exact_version: None,
        }
    }

    pub fn exact(version: Version) -> Range {
        Range {
            source: version.to_file_string(),
            sets: vec![BoundSet::exact(version.clone()).expect("Exact bound should be valid")],
            exact_version: Some(version),
        }
    }

    pub fn kind(&self) -> Option<RangeKind> {
        match self.source.chars().next() {
            Some('0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9') => {
                Some(RangeKind::Exact)
            }

            Some('^') => Some(RangeKind::Caret),
            Some('~') => Some(RangeKind::Tilde),

            _ => None,
        }
    }

    pub fn check(&self, version: &Version) -> bool {
        if version.rc.is_some() {
            let mut prerelease_match = false;
            for set in &self.sets {
                if bound_has_matching_prerelease(&set.lower, version)
                    || bound_has_matching_prerelease(&set.upper, version)
                {
                    prerelease_match = true;
                    break;
                }
            }

            if !prerelease_match {
                return false;
            }
        }

        self.sets.iter().any(|set| set.satisfies(version))
    }

    pub fn check_ignore_rc<P: Borrow<Version>>(&self, version: P) -> bool {
        self.sets.iter().any(|set| set.satisfies(version.borrow()))
    }

    pub fn exact_version(&self) -> Option<Version> {
        self.exact_version.clone()
    }

    pub fn range_min(&self) -> Option<Version> {
        if self.sets.iter().any(|set| set.is_unbounded()) {
            let min_version = Version::new_from_components(0, 0, 0, None);
            if self.check(&min_version) {
                return Some(min_version);
            }
        }

        self.sets
            .iter()
            .filter_map(|set| set.min_candidate())
            .filter(|version| self.check(version))
            .min()
    }
}

fn bound_has_matching_prerelease(bound: &Bound, version: &Version) -> bool {
    let pred = match bound {
        Bound::Lower(pred) | Bound::Upper(pred) => pred,
        Bound::Unbounded => return false,
    };

    let pred_version = pred.version();
    pred_version.rc.is_some()
        && pred_version.major == version.major
        && pred_version.minor == version.minor
        && pred_version.patch == version.patch
}

fn token_to_boundset(op: OperatorType, version: Version) -> BoundSet {
    match op {
        OperatorType::Equal => BoundSet::exact(version).expect("Exact bound should be valid"),
        OperatorType::GreaterThan => BoundSet::at_least(Predicate::Excluding(version))
            .expect("Lower bound should be valid"),
        OperatorType::GreaterThanOrEqual => BoundSet::at_least(Predicate::Including(version))
            .expect("Lower bound should be valid"),
        OperatorType::LessThan => BoundSet::at_most(Predicate::Excluding(version))
            .expect("Upper bound should be valid"),
        OperatorType::LessThanOrEqual => BoundSet::at_most(Predicate::Including(version))
            .expect("Upper bound should be valid"),
    }
}

impl FromFileString for Range {
    type Error = Error;

    fn from_file_string(src: &str) -> Result<Self, Error> {
        let tokens = Range::tokenize(src).ok_or_else(|| Error::InvalidRange(src.to_string()))?;

        let prefix = extract::infix_to_prefix(&tokens)
            .ok_or_else(|| Error::InvalidRange(src.to_string()))?;

        let sets = Range::build_sets(&prefix).ok_or_else(|| Error::InvalidRange(src.to_string()))?;

        let exact_version = if prefix.len() == 1 {
            match &prefix[0] {
                Token::Operation(OperatorType::Equal, version) => Some(version.clone()),
                _ => None,
            }
        } else {
            None
        };

        Ok(Range {
            source: src.to_string(),
            sets,
            exact_version,
        })
    }
}

impl ToFileString for Range {
    fn to_file_string(&self) -> String {
        self.source.clone()
    }
}

impl ToHumanString for Range {
    fn to_print_string(&self) -> String {
        DataType::Range.colorize(&self.to_file_string())
    }
}

impl_file_string_from_str!(Range);
impl_file_string_serialization!(Range);
