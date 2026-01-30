use ecow::{EcoString, EcoVec};
use rkyv::{
    rancor::{Fallible, Source},
    ser::{Allocator, Writer},
    string::{ArchivedString, StringResolver},
    vec::{ArchivedVec, VecResolver},
    with::{ArchiveWith, DeserializeWith, SerializeWith},
    Deserialize, DeserializeUnsized, Place, Serialize, SerializeUnsized,
};

#[derive(Debug)]
pub struct EcowAsString;

impl ArchiveWith<EcoString> for EcowAsString {
    type Archived = ArchivedString;
    type Resolver = StringResolver;

    fn resolve_with(field: &EcoString, resolver: Self::Resolver, out: Place<Self::Archived>) {
        ArchivedString::resolve_from_str(field.as_str(), resolver, out);
    }
}

impl<S: Fallible + ?Sized> SerializeWith<EcoString, S> for EcowAsString
where
    S::Error: Source,
    str: SerializeUnsized<S>,
{
    fn serialize_with(
        field: &EcoString,
        serializer: &mut S,
    ) -> Result<Self::Resolver, S::Error> {
        ArchivedString::serialize_from_str(field.as_str(), serializer)
    }
}

impl<D: Fallible + ?Sized> DeserializeWith<ArchivedString, EcoString, D> for EcowAsString
where
    str: DeserializeUnsized<str, D>,
{
    fn deserialize_with(
        field: &ArchivedString,
        _: &mut D,
    ) -> Result<EcoString, D::Error> {
        Ok(EcoString::from(field.as_str()))
    }
}

#[derive(Debug)]
pub struct EcowVec;

impl<T: rkyv::Archive> ArchiveWith<EcoVec<T>> for EcowVec {
    type Archived = ArchivedVec<T::Archived>;
    type Resolver = VecResolver;

    fn resolve_with(field: &EcoVec<T>, resolver: Self::Resolver, out: Place<Self::Archived>) {
        ArchivedVec::resolve_from_slice(field.as_slice(), resolver, out);
    }
}

impl<T, S> SerializeWith<EcoVec<T>, S> for EcowVec
where
    T: rkyv::Archive + Serialize<S>,
    S: Fallible + Allocator + Writer + ?Sized,
{
    fn serialize_with(
        field: &EcoVec<T>,
        serializer: &mut S,
    ) -> Result<Self::Resolver, S::Error> {
        ArchivedVec::<T::Archived>::serialize_from_slice(field.as_slice(), serializer)
    }
}

impl<T, D> DeserializeWith<ArchivedVec<T::Archived>, EcoVec<T>, D> for EcowVec
where
    T: rkyv::Archive + Clone,
    [T::Archived]: DeserializeUnsized<[T], D>,
    D: Fallible + ?Sized,
    D::Error: Source,
{
    fn deserialize_with(
        field: &ArchivedVec<T::Archived>,
        deserializer: &mut D,
    ) -> Result<EcoVec<T>, D::Error> {
        let values: Vec<T> = field.deserialize(deserializer)?;
        Ok(EcoVec::from(values))
    }
}
