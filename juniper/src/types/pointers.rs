use ast::{Selection, InputValue, ToInputValue, FromInputValue};
use value::Value;

use schema::meta::MetaType;
use executor::{Executor, Registry, ExecutionResult};
use types::base::{Arguments, GraphQLType};

impl<T, CtxT> GraphQLType for Box<T> where T: GraphQLType<Context=CtxT> {
    type Context = CtxT;
    type TypeInfo = T::TypeInfo;

    fn name(info: &T::TypeInfo) -> Option<&str> {
        T::name(info)
    }

    fn meta<'r>(info: &T::TypeInfo, registry: &mut Registry<'r>) -> MetaType<'r> {
        T::meta(info, registry)
    }

    fn resolve_into_type(&self, info: &T::TypeInfo, name: &str, selection_set: Option<&[Selection]>, executor: &Executor<CtxT>) -> ExecutionResult {
        (**self).resolve_into_type(info, name, selection_set, executor)
    }

    fn resolve_field(&self, info: &T::TypeInfo, field: &str, args: &Arguments, executor: &Executor<CtxT>) -> ExecutionResult
    {
        (**self).resolve_field(info, field, args, executor)
    }

    fn resolve(&self, info: &T::TypeInfo, selection_set: Option<&[Selection]>, executor: &Executor<CtxT>) -> Value {
        (**self).resolve(info, selection_set, executor)
    }
}

impl<T> FromInputValue for Box<T> where T: FromInputValue {
    fn from(v: &InputValue) -> Option<Box<T>> {
        match <T as FromInputValue>::from(v) {
            Some(v) => Some(Box::new(v)),
            None => None,
        }
    }
}

impl<T> ToInputValue for Box<T> where T: ToInputValue {
    fn to(&self) -> InputValue {
        (**self).to()
    }
}

impl<'a, T, CtxT> GraphQLType for &'a T where T: GraphQLType<Context=CtxT> {
    type Context = CtxT;
    type TypeInfo = T::TypeInfo;

    fn name(info: &T::TypeInfo) -> Option<&str> {
        T::name(info)
    }

    fn meta<'r>(info: &T::TypeInfo, registry: &mut Registry<'r>) -> MetaType<'r> {
        T::meta(info, registry)
    }

    fn resolve_into_type(&self, info: &T::TypeInfo, name: &str, selection_set: Option<&[Selection]>, executor: &Executor<CtxT>) -> ExecutionResult {
        (**self).resolve_into_type(info, name, selection_set, executor)
    }

    fn resolve_field(&self, info: &T::TypeInfo, field: &str, args: &Arguments, executor: &Executor<CtxT>) -> ExecutionResult
    {
        (**self).resolve_field(info, field, args, executor)
    }

    fn resolve(&self, info: &T::TypeInfo, selection_set: Option<&[Selection]>, executor: &Executor<CtxT>) -> Value {
        (**self).resolve(info, selection_set, executor)
    }
}

impl<'a, T> ToInputValue for &'a T where T: ToInputValue {
    fn to(&self) -> InputValue {
        (**self).to()
    }
}
