/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

// BSL (1C:Enterprise Script) grammar for tree-sitter
// Based on: 1C:Enterprise 8.3.27 Developer Guide, Chapter 4

// Helper: case-insensitive keyword for Russian + English
const ci = (/** @type {string} */ word) => {
  let result = '';
  for (const ch of word) {
    const lower = ch.toLowerCase();
    const upper = ch.toUpperCase();
    if (lower !== upper) {
      result += `[${lower}${upper}]`;
    } else {
      result += ch;
    }
  }
  return new RegExp(result);
};

const PREC = {
  OR: 1,
  AND: 2,
  NOT: 3,
  COMPARE: 4,
  ADD: 5,
  MUL: 6,
  UNARY: 7,
  MEMBER: 8,
};

module.exports = grammar({
  name: 'bsl',

  extras: $ => [/\s/, $.comment],

  word: $ => $.identifier,

  conflicts: $ => [
    [$.primary_expression, $.lvalue],
    [$._definition, $.procedure_declaration, $.function_declaration],
    [$._postfix_expression, $.lvalue],
  ],

  rules: {
    source_file: $ => repeat($._definition),

    _definition: $ => choice(
      $.annotation,
      $.region,
      $.preprocessor_directive,
      $.var_declaration,
      $.procedure_declaration,
      $.function_declaration,
      $._statement,
    ),

    // Comments: only // line comments in BSL
    comment: _ => token(seq('//', /.*/)),

    // Identifiers: Latin + Cyrillic + underscore
    identifier: _ => /[A-Za-z\u0410-\u044F\u0401\u0451_][A-Za-z0-9\u0410-\u044F\u0401\u0451_]*/,

    // === Annotations (compilation directives & extension annotations) ===
    annotation: $ => prec.left(seq(
      '&',
      field('name', $.annotation_name),
      optional(seq('(', $.string, ')')),
    )),

    annotation_name: _ => token(choice(
      // Compilation directives (Russian)
      ci('НаКлиенте'),
      ci('НаСервере'),
      ci('НаСервереБезКонтекста'),
      ci('НаКлиентеНаСервереБезКонтекста'),
      ci('НаКлиентеНаСервере'),
      // Compilation directives (English)
      ci('AtClient'),
      ci('AtServer'),
      ci('AtServerNoContext'),
      ci('AtClientAtServerNoContext'),
      ci('AtClientAtServer'),
      // Extension annotations (Russian)
      ci('Перед'),
      ci('После'),
      ci('Вместо'),
      ci('ИзменениеИКонтроль'),
      // Extension annotations (English)
      ci('Before'),
      ci('After'),
      ci('Around'),
      ci('ChangeAndValidate'),
    )),

    // === Regions ===
    region: $ => seq(
      $.region_start,
      repeat($._definition),
      $.region_end,
    ),

    region_start: $ => seq(
      '#',
      token(choice(ci('Область'), ci('Region'))),
      field('name', $.identifier),
    ),

    region_end: _ => seq(
      '#',
      token(choice(ci('КонецОбласти'), ci('EndRegion'))),
    ),

    // === Preprocessor directives ===
    preprocessor_directive: $ => choice(
      $.preprocessor_if,
      $.preprocessor_elsif,
      $.preprocessor_else,
      $.preprocessor_endif,
      $.preprocessor_insert,
      $.preprocessor_endinsert,
      $.preprocessor_delete,
      $.preprocessor_enddelete,
    ),

    preprocessor_if: $ => seq('#', token(choice(ci('Если'), ci('If'))), $.preprocessor_expression, token(choice(ci('Тогда'), ci('Then')))),
    preprocessor_elsif: $ => seq('#', token(choice(ci('ИначеЕсли'), ci('ElsIf'))), $.preprocessor_expression, token(choice(ci('Тогда'), ci('Then')))),
    preprocessor_else: _ => seq('#', token(choice(ci('Иначе'), ci('Else')))),
    preprocessor_endif: _ => seq('#', token(choice(ci('КонецЕсли'), ci('EndIf')))),
    preprocessor_insert: _ => seq('#', token(choice(ci('Вставка'), ci('Insert')))),
    preprocessor_endinsert: _ => seq('#', token(choice(ci('КонецВставки'), ci('EndInsert')))),
    preprocessor_delete: _ => seq('#', token(choice(ci('Удаление'), ci('Delete')))),
    preprocessor_enddelete: _ => seq('#', token(choice(ci('КонецУдаления'), ci('EndDelete')))),

    preprocessor_expression: $ => choice(
      $.preprocessor_symbol,
      $.preprocessor_not,
      $.preprocessor_and,
      $.preprocessor_or,
      seq('(', $.preprocessor_expression, ')'),
    ),

    preprocessor_symbol: _ => token(choice(
      ci('Клиент'), ci('Client'),
      ci('НаКлиенте'), ci('AtClient'),
      ci('НаСервере'), ci('AtServer'),
      ci('Сервер'), ci('Server'),
      ci('ВнешнееСоединение'), ci('ExternalConnection'),
      ci('ТонкийКлиент'), ci('ThinClient'),
      ci('ВебКлиент'), ci('WebClient'),
      ci('МобильноеПриложениеКлиент'), ci('MobileAppClient'),
      ci('МобильноеПриложениеСервер'), ci('MobileAppServer'),
      ci('МобильныйКлиент'), ci('MobileClient'),
    )),

    preprocessor_not: $ => prec(3, seq(token(choice(ci('Не'), ci('Not'))), $.preprocessor_expression)),
    preprocessor_and: $ => prec.left(2, seq($.preprocessor_expression, token(choice(ci('И'), ci('And'))), $.preprocessor_expression)),
    preprocessor_or: $ => prec.left(1, seq($.preprocessor_expression, token(choice(ci('ИЛИ'), ci('Or'))), $.preprocessor_expression)),

    // === Variable declarations ===
    var_declaration: $ => seq(
      token(choice(ci('Перем'), ci('Var'))),
      $.var_name,
      repeat(seq(',', $.var_name)),
      optional(token(choice(ci('Экспорт'), ci('Export')))),
      ';',
    ),

    var_name: $ => field('name', $.identifier),

    // === Procedure declaration (P7: with optional Async) ===
    procedure_declaration: $ => seq(
      repeat($.annotation),
      optional(field('async', token(choice(ci('Асинх'), ci('Async'))))),
      token(choice(ci('Процедура'), ci('Procedure'))),
      field('name', $.identifier),
      '(',
      optional($.parameter_list),
      ')',
      optional(field('export', token(choice(ci('Экспорт'), ci('Export'))))),
      repeat($._body_item),
      token(choice(ci('КонецПроцедуры'), ci('EndProcedure'))),
    ),

    // === Function declaration (P7: with optional Async) ===
    function_declaration: $ => seq(
      repeat($.annotation),
      optional(field('async', token(choice(ci('Асинх'), ci('Async'))))),
      token(choice(ci('Функция'), ci('Function'))),
      field('name', $.identifier),
      '(',
      optional($.parameter_list),
      ')',
      optional(field('export', token(choice(ci('Экспорт'), ci('Export'))))),
      repeat($._body_item),
      token(choice(ci('КонецФункции'), ci('EndFunction'))),
    ),

    parameter_list: $ => seq(
      $.parameter,
      repeat(seq(',', $.parameter)),
    ),

    parameter: $ => seq(
      optional(token(choice(ci('Знач'), ci('Val')))),
      field('name', $.identifier),
      optional(seq('=', $._expression)),
    ),

    // === Body items ===
    _body_item: $ => choice(
      $.var_declaration,
      $._statement,
      $.preprocessor_directive,
      $.region,
    ),

    _statement: $ => choice(
      $.assignment_statement,
      $.call_statement,
      $.return_statement,
      $.if_statement,
      $.while_statement,
      $.for_statement,
      $.for_each_statement,
      $.try_statement,
      $.raise_statement,
      $.execute_statement,
      $.goto_statement,
      $.label_statement,
      $.continue_statement,
      $.break_statement,
      $.add_handler_statement,
      $.remove_handler_statement,
      $.await_statement,
      ';',
    ),

    // === Statements ===
    assignment_statement: $ => seq($.lvalue, '=', $._expression, ';'),

    call_statement: $ => seq($._callable_expression, ';'),

    _callable_expression: $ => choice(
      $.method_call,
      $.member_method_call,
    ),

    method_call: $ => prec(PREC.MEMBER, seq(
      field('name', $.identifier),
      '(',
      optional($.argument_list),
      ')',
    )),

    member_method_call: $ => prec.left(PREC.MEMBER, seq(
      field('object', $._expression),
      '.',
      field('method', $.identifier),
      '(',
      optional($.argument_list),
      ')',
    )),

    return_statement: $ => seq(
      token(choice(ci('Возврат'), ci('Return'))),
      optional($._expression),
      ';',
    ),

    if_statement: $ => seq(
      token(choice(ci('Если'), ci('If'))),
      field('condition', $._expression),
      token(choice(ci('Тогда'), ci('Then'))),
      repeat($._body_item),
      repeat($.elsif_clause),
      optional($.else_clause),
      token(choice(ci('КонецЕсли'), ci('EndIf'))),
    ),

    elsif_clause: $ => seq(
      token(choice(ci('ИначеЕсли'), ci('ElsIf'))),
      field('condition', $._expression),
      token(choice(ci('Тогда'), ci('Then'))),
      repeat($._body_item),
    ),

    else_clause: $ => seq(
      token(choice(ci('Иначе'), ci('Else'))),
      repeat($._body_item),
    ),

    while_statement: $ => seq(
      token(choice(ci('Пока'), ci('While'))),
      field('condition', $._expression),
      token(choice(ci('Цикл'), ci('Do'))),
      repeat($._body_item),
      token(choice(ci('КонецЦикла'), ci('EndDo'))),
    ),

    for_statement: $ => seq(
      token(choice(ci('Для'), ci('For'))),
      field('variable', $.identifier),
      '=',
      field('start', $._expression),
      token(choice(ci('По'), ci('To'))),
      field('end', $._expression),
      token(choice(ci('Цикл'), ci('Do'))),
      repeat($._body_item),
      token(choice(ci('КонецЦикла'), ci('EndDo'))),
    ),

    for_each_statement: $ => seq(
      token(choice(ci('Для'), ci('For'))),
      token(choice(ci('Каждого'), ci('Each'))),
      field('variable', $.identifier),
      token(choice(ci('Из'), ci('In'))),
      field('collection', $._expression),
      token(choice(ci('Цикл'), ci('Do'))),
      repeat($._body_item),
      token(choice(ci('КонецЦикла'), ci('EndDo'))),
    ),

    try_statement: $ => seq(
      token(choice(ci('Попытка'), ci('Try'))),
      repeat($._body_item),
      token(choice(ci('Исключение'), ci('Except'))),
      repeat($._body_item),
      token(choice(ci('КонецПопытки'), ci('EndTry'))),
    ),

    raise_statement: $ => seq(
      token(choice(ci('ВызватьИсключение'), ci('Raise'))),
      optional($._expression),
      ';',
    ),

    execute_statement: $ => seq(
      token(choice(ci('Выполнить'), ci('Execute'))),
      '(',
      $._expression,
      ')',
      ';',
    ),

    goto_statement: $ => seq(
      token(choice(ci('Перейти'), ci('Goto'))),
      '~',
      $.identifier,
      ';',
    ),

    label_statement: $ => seq('~', $.identifier, ':'),

    continue_statement: _ => seq(
      token(choice(ci('Продолжить'), ci('Continue'))),
      ';',
    ),

    break_statement: _ => seq(
      token(choice(ci('Прервать'), ci('Break'))),
      ';',
    ),

    add_handler_statement: $ => seq(
      token(choice(ci('ДобавитьОбработчик'), ci('AddHandler'))),
      field('event', $._expression),
      ',',
      field('handler', $._expression),
      ';',
    ),

    remove_handler_statement: $ => seq(
      token(choice(ci('УдалитьОбработчик'), ci('RemoveHandler'))),
      field('event', $._expression),
      ',',
      field('handler', $._expression),
      ';',
    ),

    await_statement: $ => seq(
      token(choice(ci('Ждать'), ci('Await'))),
      $._expression,
      ';',
    ),

    // === Expressions (precedence climbing) ===
    _expression: $ => choice(
      $.or_expression,
      $.and_expression,
      $.not_expression,
      $.comparison_expression,
      $.additive_expression,
      $.multiplicative_expression,
      $.unary_expression,
      $._postfix_expression,
    ),

    or_expression: $ => prec.left(PREC.OR, seq($._expression, token(choice(ci('ИЛИ'), ci('Или'), ci('Or'), ci('OR'))), $._expression)),
    and_expression: $ => prec.left(PREC.AND, seq($._expression, token(choice(ci('И'), ci('And'), ci('AND'))), $._expression)),
    not_expression: $ => prec(PREC.NOT, seq(token(choice(ci('Не'), ci('Not'), ci('NOT'))), $._expression)),

    comparison_expression: $ => prec.left(PREC.COMPARE, seq($._expression, choice('=', '<>', '<', '>', '<=', '>='), $._expression)),
    additive_expression: $ => prec.left(PREC.ADD, seq($._expression, choice('+', '-'), $._expression)),
    multiplicative_expression: $ => prec.left(PREC.MUL, seq($._expression, choice('*', '/', '%'), $._expression)),
    unary_expression: $ => prec(PREC.UNARY, seq(choice('+', '-'), $._expression)),

    // Postfix expressions: member access, method call, index access
    _postfix_expression: $ => choice(
      $.member_access,
      $.member_method_call,
      $.index_access,
      $.primary_expression,
    ),

    primary_expression: $ => choice(
      $.identifier,
      $.number,
      $.string,
      $.multiline_string,
      $.date_literal,
      $.boolean,
      $.undefined,
      $.null,
      $.method_call,
      $.parenthesized_expression,
      $.ternary_expression,
      $.new_expression,
    ),

    // === Lvalue ===
    lvalue: $ => choice(
      $.identifier,
      $.member_access,
      $.index_access,
    ),

    member_access: $ => prec.left(PREC.MEMBER, seq(
      field('object', $._postfix_expression),
      '.',
      field('property', $.identifier),
    )),

    index_access: $ => prec.left(PREC.MEMBER, seq(
      field('object', $._postfix_expression),
      '[',
      field('index', $._expression),
      ']',
    )),

    parenthesized_expression: $ => seq('(', $._expression, ')'),

    ternary_expression: $ => prec.right(0, seq('?', '(', $._expression, ',', $._expression, ',', $._expression, ')')),

    new_expression: $ => seq(
      token(choice(ci('Новый'), ci('New'))),
      field('type', $.identifier),
      optional(seq('(', optional($.argument_list), ')')),
    ),

    argument_list: $ => choice(
      seq($._expression, repeat(seq(',', optional($._expression)))),
      seq(',', optional($._expression), repeat(seq(',', optional($._expression)))),
    ),

    // === Literals ===
    string: _ => seq('"', repeat(choice(/[^"\n]/, '""')), '"'),

    multiline_string: _ => token(seq(
      '"', /[^"\n]*/, '\n',
      repeat(seq(/\s*\|/, /[^"\n]*/, optional('\n'))),
      /\s*/, '"',
    )),

    boolean: _ => token(choice(
      ci('Истина'), ci('True'),
      ci('Ложь'), ci('False'),
    )),

    number: _ => /\d+(\.\d+)?/,

    date_literal: _ => seq('\'', /\d{8,14}/, '\''),

    undefined: _ => token(choice(ci('Неопределено'), ci('Undefined'))),

    null: _ => /[Nn][Uu][Ll][Ll]/,
  },
});
