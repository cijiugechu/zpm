use std::str::FromStr;

use crate::Path;

pub struct PathIterator<'a> {
    components: Vec<&'a str>,
    front_idx: usize,
    back_idx: usize,
    has_trailing_slash: bool,
}

impl<'a> PathIterator<'a> {
    pub fn new(path: &'a Path) -> Self {
        let path_str = path.as_str();

        let has_leading_slash = path_str.starts_with('/');
        let has_trailing_slash = path_str.ends_with('/') && path_str.len() > 1;

        let components_path = path_str;
        let components_path = components_path.strip_prefix('/').unwrap_or(components_path);
        let components_path = components_path.strip_suffix('/').unwrap_or(components_path);

        let mut components = Vec::new();

        if has_leading_slash {
            components.push("");
        }

        if components_path.len() > 0 {
            components.extend(components_path.split('/'));
        }

        let front_idx = 1;
        let back_idx = components.len();

        Self {
            components,
            front_idx,
            back_idx,
            has_trailing_slash,
        }
    }
}

impl<'a> Iterator for PathIterator<'a> {
    type Item = Path;

    fn next(&mut self) -> Option<Self::Item> {
        if self.front_idx > self.back_idx {
            return None;
        }

        let front_idx = self.front_idx;
        self.front_idx += 1;

        if front_idx == 1 {
            return Some(Path::root());
        }

        let mut path
            = self.components[0..front_idx].join("/");

        if self.has_trailing_slash {
            path.push('/');
        }

        Some(Path::from_str(&path).unwrap())
    }
}

impl<'a> DoubleEndedIterator for PathIterator<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.front_idx > self.back_idx {
            return None;
        }

        let back_idx = self.back_idx;
        self.back_idx -= 1;

        if back_idx == 1 {
            return Some(Path::root());
        }

        let components
            = self.components[0..back_idx].join("/");

        Some(Path::from_str(&components).unwrap())
    }
}
