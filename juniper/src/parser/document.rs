use std::borrow::Cow;

use ast::{Definition, Document, OperationType,
          VariableDefinitions, VariableDefinition, InputValue,
          Operation, Fragment, Selection, Directive, Field, Arguments,
          FragmentSpread, InlineFragment, Type};

use parser::{Lexer, Parser, Spanning, UnlocatedParseResult, OptionParseResult, ParseResult, ParseError, Token};
use parser::value::parse_value_literal;

#[doc(hidden)]
pub fn parse_document_source(s: &str) -> UnlocatedParseResult<Document> {
    let mut lexer = Lexer::new(s);
    let mut parser = try!(Parser::new(&mut lexer).map_err(|s| s.map(ParseError::LexerError)));
    parse_document(&mut parser)
}

fn parse_document<'a>(parser: &mut Parser<'a>) -> UnlocatedParseResult<'a, Document<'a>> {
    let mut defs = Vec::new();

    loop {
        defs.push(try!(parse_definition(parser)));

        if parser.peek().item == Token::EndOfFile {
            return Ok(defs);
        }
    }
}

fn parse_definition<'a>(parser: &mut Parser<'a>) -> UnlocatedParseResult<'a, Definition<'a>> {
    match parser.peek().item {
        Token::CurlyOpen | Token::Name("query") | Token::Name("mutation") =>
            Ok(Definition::Operation(try!(parse_operation_definition(parser)))),
        Token::Name("fragment") =>
            Ok(Definition::Fragment(try!(parse_fragment_definition(parser)))),
        _ => Err(parser.next()?.map(ParseError::UnexpectedToken)),
    }
}

fn parse_operation_definition<'a>(parser: &mut Parser<'a>) -> ParseResult<'a, Operation<'a>> {
    if parser.peek().item == Token::CurlyOpen {
        let selection_set = try!(parse_selection_set(parser));

        Ok(Spanning::start_end(
            &selection_set.start,
            &selection_set.end,
            Operation {
                operation_type: OperationType::Query,
                name: None,
                variable_definitions: None,
                directives: None,
                selection_set: selection_set.item,
            }))
    }
    else {
        let start_pos = parser.peek().start.clone();
        let operation_type = try!(parse_operation_type(parser));
        let name = match parser.peek().item {
            Token::Name(_) => Some(try!(parser.expect_name())),
            _ => None
        };
        let variable_definitions = try!(parse_variable_definitions(parser));
        let directives = try!(parse_directives(parser));
        let selection_set = try!(parse_selection_set(parser));

        Ok(Spanning::start_end(
            &start_pos,
            &selection_set.end,
            Operation {
                operation_type: operation_type.item,
                name: name,
                variable_definitions: variable_definitions,
                directives: directives.map(|s| s.item),
                selection_set: selection_set.item,
            }))
    }
}

fn parse_fragment_definition<'a>(parser: &mut Parser<'a>) -> ParseResult<'a, Fragment<'a>> {
    let Spanning { start: start_pos, .. } = try!(parser.expect(&Token::Name("fragment")));
    let name = match parser.expect_name() {
        Ok(n) => if n.item == "on" {
                return Err(n.map(|_| ParseError::UnexpectedToken(Token::Name("on"))));
            }
            else {
                n
            },
        Err(e) => return Err(e),
    };

    try!(parser.expect(&Token::Name("on")));
    let type_cond = try!(parser.expect_name());
    let directives = try!(parse_directives(parser));
    let selection_set = try!(parse_selection_set(parser));

    Ok(Spanning::start_end(
        &start_pos,
        &selection_set.end,
        Fragment {
            name: name,
            type_condition: type_cond,
            directives: directives.map(|s| s.item),
            selection_set: selection_set.item,
        }))
}

fn parse_optional_selection_set<'a>(parser: &mut Parser<'a>) -> OptionParseResult<'a, Vec<Selection<'a>>> {
    if parser.peek().item == Token::CurlyOpen {
        Ok(Some(try!(parse_selection_set(parser))))
    }
    else {
        Ok(None)
    }
}

fn parse_selection_set<'a>(parser: &mut Parser<'a>) -> ParseResult<'a, Vec<Selection<'a>>> {
    parser.unlocated_delimited_nonempty_list(
        &Token::CurlyOpen,
        parse_selection,
        &Token::CurlyClose)
}

fn parse_selection<'a>(parser: &mut Parser<'a>) -> UnlocatedParseResult<'a, Selection<'a>> {
    match parser.peek().item {
        Token::Ellipsis => parse_fragment(parser),
        _ => parse_field(parser).map(Selection::Field),
    }
}

fn parse_fragment<'a>(parser: &mut Parser<'a>) -> UnlocatedParseResult<'a, Selection<'a>> {
    let Spanning { start: ref start_pos, .. } = try!(parser.expect(&Token::Ellipsis));

    match parser.peek().item {
        Token::Name("on") => {
            parser.next()?;
            let name = try!(parser.expect_name());
            let directives = try!(parse_directives(parser));
            let selection_set = try!(parse_selection_set(parser));

            Ok(Selection::InlineFragment(
                Spanning::start_end(
                    &start_pos.clone(),
                    &selection_set.end,
                    InlineFragment {
                        type_condition: Some(name),
                        directives: directives.map(|s| s.item),
                        selection_set: selection_set.item,
                    })))
        },
        Token::CurlyOpen => {
            let selection_set = try!(parse_selection_set(parser));

            Ok(Selection::InlineFragment(
                Spanning::start_end(
                    &start_pos.clone(),
                    &selection_set.end,
                    InlineFragment {
                        type_condition: None,
                        directives: None,
                        selection_set: selection_set.item,
                    })))
        },
        Token::Name(_) => {
            let frag_name = try!(parser.expect_name());
            let directives = try!(parse_directives(parser));

            Ok(Selection::FragmentSpread(
                Spanning::start_end(
                    &start_pos.clone(),
                    &directives.as_ref().map_or(&frag_name.end, |s| &s.end).clone(),
                    FragmentSpread {
                        name: frag_name,
                        directives: directives.map(|s| s.item),
                    })))
        },
        Token::At => {
            let directives = try!(parse_directives(parser));
            let selection_set = try!(parse_selection_set(parser));

            Ok(Selection::InlineFragment(
                Spanning::start_end(
                    &start_pos.clone(),
                    &selection_set.end,
                    InlineFragment {
                        type_condition: None,
                        directives: directives.map(|s| s.item),
                        selection_set: selection_set.item,
                    })))
        },
        _ => Err(parser.next()?.map(ParseError::UnexpectedToken)),
    }
}

fn parse_field<'a>(parser: &mut Parser<'a>) -> ParseResult<'a, Field<'a>> {
    let mut alias = Some(try!(parser.expect_name()));

    let name = if try!(parser.skip(&Token::Colon)).is_some() {
        try!(parser.expect_name())
    }
    else {
        alias.take().unwrap()
    };

    let arguments = try!(parse_arguments(parser));
    let directives = try!(parse_directives(parser));
    let selection_set = try!(parse_optional_selection_set(parser));

    Ok(Spanning::start_end(
        &alias.as_ref().unwrap_or(&name).start.clone(),
        &selection_set.as_ref().map(|s| &s.end)
            .or_else(|| directives.as_ref().map(|s| &s.end))
            .or_else(|| arguments.as_ref().map(|s| &s.end))
            .unwrap_or(&name.end)
            .clone(),
        Field {
            alias: alias,
            name: name,
            arguments: arguments,
            directives: directives.map(|s| s.item),
            selection_set: selection_set.map(|s| s.item),
        }))
}

fn parse_arguments<'a>(parser: &mut Parser<'a>) -> OptionParseResult<'a, Arguments<'a>> {
    if parser.peek().item != Token::ParenOpen {
        Ok(None)
    } else {
        Ok(Some(try!(parser.delimited_nonempty_list(
                &Token::ParenOpen,
                parse_argument,
                &Token::ParenClose
            )).map(|args| Arguments { items: args.into_iter().map(|s| s.item).collect() })))
    }
}

fn parse_argument<'a>(parser: &mut Parser<'a>) -> ParseResult<'a, (Spanning<&'a str>, Spanning<InputValue>)> {
    let name = try!(parser.expect_name());
    try!(parser.expect(&Token::Colon));
    let value = try!(parse_value_literal(parser, false));

    Ok(Spanning::start_end(
        &name.start.clone(),
        &value.end.clone(),
        (name, value)))
}

fn parse_operation_type<'a>(parser: &mut Parser<'a>) -> ParseResult<'a, OperationType> {
    match parser.peek().item {
        Token::Name("query") => Ok(parser.next()?.map(|_| OperationType::Query)),
        Token::Name("mutation") => Ok(parser.next()?.map(|_| OperationType::Mutation)),
        _ => Err(parser.next()?.map(ParseError::UnexpectedToken))
    }
}

fn parse_variable_definitions<'a>(parser: &mut Parser<'a>) -> OptionParseResult<'a, VariableDefinitions<'a>> {
    if parser.peek().item != Token::ParenOpen {
        Ok(None)
    }
    else {
        Ok(Some(try!(parser.delimited_nonempty_list(
                &Token::ParenOpen,
                parse_variable_definition,
                &Token::ParenClose
            )).map(|defs| VariableDefinitions { items: defs.into_iter().map(|s| s.item).collect() })))
    }
}

fn parse_variable_definition<'a>(parser: &mut Parser<'a>) -> ParseResult<'a, (Spanning<&'a str>, VariableDefinition<'a>)> {
    let Spanning { start: start_pos, .. } = try!(parser.expect(&Token::Dollar));
    let var_name = try!(parser.expect_name());
    try!(parser.expect(&Token::Colon));
    let var_type = try!(parse_type(parser));

    let default_value = if try!(parser.skip(&Token::Equals)).is_some() {
            Some(try!(parse_value_literal(parser, true)))
        }
        else {
            None
        };

    Ok(Spanning::start_end(
        &start_pos,
        &default_value.as_ref().map_or(&var_type.end, |s| &s.end).clone(),
        (
            Spanning::start_end(
                &start_pos,
                &var_name.end,
                var_name.item,
            ),
            VariableDefinition {
                var_type: var_type,
                default_value: default_value,
            }
        )))
}

fn parse_directives<'a>(parser: &mut Parser<'a>) -> OptionParseResult<'a, Vec<Spanning<Directive<'a>>>> {
    if parser.peek().item != Token::At {
        Ok(None)
    }
    else {
        let mut items = Vec::new();
        while parser.peek().item == Token::At {
            items.push(try!(parse_directive(parser)));
        }

        Ok(Spanning::spanning(items))
    }
}

fn parse_directive<'a>(parser: &mut Parser<'a>) -> ParseResult<'a, Directive<'a>> {
    let Spanning { start: start_pos, .. } = try!(parser.expect(&Token::At));
    let name = try!(parser.expect_name());
    let arguments = try!(parse_arguments(parser));

    Ok(Spanning::start_end(
        &start_pos,
        &arguments.as_ref().map_or(&name.end, |s| &s.end).clone(),
        Directive {
            name: name,
            arguments: arguments,
        }))
}

pub fn parse_type<'a>(parser: &mut Parser<'a>) -> ParseResult<'a, Type<'a>> {
    let parsed_type = if let Some(Spanning { start: start_pos, ..}) = try!(parser.skip(&Token::BracketOpen)) {
        let inner_type = try!(parse_type(parser));
        let Spanning { end: end_pos, .. } = try!(parser.expect(&Token::BracketClose));
        Spanning::start_end(
            &start_pos,
            &end_pos,
            Type::List(Box::new(inner_type.item)))
    }
    else {
        try!(parser.expect_name()).map(|s| Type::Named(Cow::Borrowed(s)))
    };

    Ok(match *parser.peek() {
        Spanning { item: Token::ExclamationMark, .. } =>
            try!(wrap_non_null(parser, parsed_type)),
        _ => parsed_type
    })
}

fn wrap_non_null<'a>(parser: &mut Parser<'a>, inner: Spanning<Type<'a>>) -> ParseResult<'a, Type<'a>> {
    let Spanning { end: end_pos, .. } = try!(parser.expect(&Token::ExclamationMark));

    let wrapped = match inner.item {
        Type::Named(name) => Type::NonNullNamed(name),
        Type::List(l) => Type::NonNullList(l),
        t => t,
    };

    Ok(Spanning::start_end(&inner.start, &end_pos, wrapped))
}
