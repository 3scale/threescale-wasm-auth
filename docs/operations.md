- [Operations Reference](#operations-reference)
  - [The `operation` object](#the-operation-object)
    - [The value stack](#the-value-stack)
    - [Decoding operations](#decoding-operations)
    - [String operations](#string-operations)
    - [Stack operations](#stack-operations)
      - [Indexing](#indexing)
      - [Operations](#operations)
    - [Check operations](#check-operations)
    - [Control operations](#control-operations)
    - [Format-specific operations](#format-specific-operations)
      - [The `structured format` objects](#the-structured-format-objects)
      - [Example 1](#example-1)
      - [Example 2](#example-2)

# Operations Reference

This document describes the available operations for `lookup queries` in `source` objects.

## The `operation` object

Each element in the `ops` array belonging to a specific `source type` is an `operation` object that
either applies transformations to values or performs tests. The field name to use for such an object
is the name of the `operation` itself, and any values are the parameters to the `operation`, which
could themselves be structure objects (ie. maps with fields and values), lists or strings.

Most `operations` consume one or more inputs, and produce one or more outputs. Whenever they consume
inputs or produce outputs, they work with a stack of values: each value consumed by the operations
is popped from the stack of values (initially populated with any `source` matches), and any values
output by them will be pushed to the stack. Some other `operations` don't consume or produce outputs
other than asserting certain properties, but they still inspect a stack of values.

*Note*: whenever resolution finishes, the values picked up by the next step (such as assigning the
values to be an `app_id`, an `app_key` or a `user_key`) are always taken from the bottom values of
the stack.

There are a few different `operations` categories:

* `decode`: these transform an input value by decoding it to obtain a different format.
* `string`: these take a (string) value as input and perform transformations and checks on it.
* `stack`: these take a set of values in the input and perform multiple stack transformations and
           selection of specific positions in the stack.
* `check`: these assert properties about sets of operations in a side-effect free way.
* `control`: these perform operations that allow for modifying the evaluation flow.
* `format`: these parse the format-specific structure of input values and look up values in it.

All operations are specified by the name identifiers as strings.

### The value stack

All non-`control` operations act on one or more values on a stack, and upon success generate one or
more values. The input of an operation is either the value in the top of the stack or the whole
stack.

Operations that take one value as input, pop it from the stack, and any outputs are pushed to the
stack _in order_. That is, an operation that produces two values, ie. splitting a string, will
generate them from left to right, and so the first value will be pushed first, then the second, and
so on, so when the next operation pops a value from the stack, it will match the last value of the
previous operation.

Operations that take multiple values as input always produce one or more values when successful.

Operations in the `control` category are special in that they evaluate one or more operations, but
they don't change the values in the stack per se. On the contrary, they are used to test for the
success or failure of other operations, and only in some particular cases the output of such
operations is picked up in the stack.

After all operations have been evaluated, a successful resolution means the stack is populated by at
least one value. When an operation fails, the failure bubbles up until it either hits the top level
of the `source` object, or a suitable `control` operation re-interprets its meaning - that is, you
could have asserted that a certain operation should fail, which upon failure, would mark that
`control` operation as successful, thus continuing evaluating the next operation.

Upon sucessful evaluation, values will be picked by the different authorization pattern variables
from the bottom of the stack. In practical terms, this means that, upon success:

* `user_key` and `app_key` will take the value in the bottom of the stack.
* `app_id` will take the value in the bottom of the stack, and will take the next value as `app_key`
  if available.

### Decoding operations

The following `decoding` operations are currently supported:

* `base64_standard`: this interprets a value as [`Base64`](https://en.wikipedia.org/wiki/Base64)
                     encoded. Note that this is _seldom the operation you want_.
* `base64_urlsafe`: this interprets the input as [`Base64 URL variant`](https://en.wikipedia.org/wiki/Base64#The_URL_applications),
                    which is the variant typically used in HTTP applications.

None of the decoding operations currently accept any parameter other than their single input value,
and they all return a single value or an error.

### String operations

String operations work on a single string value popped from the stack and produce one or more values
which are then pushed in order to the stack, except for those operations that only perform checks.

* `strlen`: checks the string for a minimum or maximum length. This operation takes three optional
            parameters: `min`, `max` and `mode`. `mode` determines the meaning of `strlen`: by
            default, or if set to the value `utf8`, it is set to count [`UTF-8`](https://en.wikipedia.org/wiki/UTF-8)
            characters. If set to `bytes`, it will count bytes. Note that `UTF-8` characters can take
            multiple bytes, so the both modes are not equivalent. If `min` is specified, the
            operation will fail if the string has less than the specified length, and conversely, if
            `max` is specified the operation will fail if the string has more than the specified
            length.
* `strrev`: takes no parameters and just reverses its string input.
* `split`: splits the string in multiple substrings starting from the beginning (typically left),
           separated by a substring taken in the `separator` parameter, which is optional and
           defaults to the string `":"`. Another optional parameter, `max`, specifies how many times,
           at most, to split the string using the separator, with a default value of `0` indicating
           no limit.
* `rsplit`: splits the string in multiple substrings starting from the end (typically right),
            separated by a substring taken in the `separator` parameter, which is optional and
            defaults to the string `":"`. Another optional parameter, `max`, specifies how many
            times, at most, to split the string using the separator, with a default value of `0`
            indicating no limit.
* `replace`: replaces any instances of a substring with another. Takes two required parameters:
             `pattern`, which indicates the substring to replace, and `with`, which specifies the
             replacement. An optional `max` parameter specifies how many times the replacement should
             happen, at most, from beginning to end (typically left to right), with a default value
             of `0` indicating no limit.
* `prefix`: checks that a given string parameter is a prefix for the topmost value in the stack. If
            the value specified to this operation is not such a prefix, the operation will fail.
* `suffix`: checks that a given string parameter is a suffix for the topmost value in the stack. If
            the value specified to this operation is not such a suffix, the operation will fail.
* `substr`: checks that a given string parameter is a substring of the input string, regardless of
            its location.
* `glob`: takes a list of string parameters indicating glob patterns, where characters `*`, `+` and
          `?` have a special meaning to indicate, respectively, _0 or more_ characters, _1 or more_
          characters, and _0 or 1_ character. Such special characters can be escaped with backward
          slashes `\`, and the backward slash itself can be escaped with another backward slash. The
          operation is successful if the pattern matches the input string.

### Stack operations

Stack operations manipulate the current stack of values. Note that some operations will run other
operations with modified stack contents in order to perform some higher order function. In the text
you'll see references to the `head` or `tail` of the stack. Some people have difficulty referring to
a stack as if it was a queue, so if you find it confusing, try mapping those concepts to the `bottom`
and the `top` of the stack, respectively.

Operations resulting in empty stacks or trying to access values out of bounds will return a failure.

Before listing the operations, it is important to know about how indexing into the stack works.

#### Indexing

Some operations accept 0-based indexes within the stack. Those have semantics similar to [`Ruby
array indexing`](https://ruby-doc.org/core-3.0.2/Array.html#class-Array-label-Array+Indexes):
you can use `0` to refer to the first element in the stack, and `-1` to refer
to the last element in the stack, `1` to refer to the second element, and `-2` to the next to
last element. Indexes equal to or above the length of the stack will result in failed operations.
Contrary to Ruby, though, negative indexes whose absolute value is equal to or above the length of
the stack have undefined behavior, that is, they might or might not work as expected, and they might
cause the operation to fail. This behavior might become well defined in a future release.

#### Operations

Here's the current list of operations:

* `length`: performs an assertion on the number of values on the stack. Receives the optional
            parameters `min` and `max`, which accept a number each and check for minimum and
            maximum number of values respectively.
* `join`: joins the stack values into a single string separated by the provided separator string.
* `reverse`: reverses the stack so that values that were in the bottom will be the topmost ones,
             and conversely the topmost values will move down to the bottom.
* `contains`: takes a string value and returns successfully if the stack contains an element with
              that value.
* `take`: takes any number of values from the bottom of the stack and any number of values from the
          top and joins them. Receives two optional parameters: `head` and `tail`, for bottom and
          top, respectively, with both accepting the number of values to take. Useful to keep only
          a certain number of values in the bottom or the top of the stack.
* `drop`: like `take`, but instead of taking the values it will drop them. Receives two optional
          parameters: `head` and `tail`, with both accepting the number of values to drop. Useful
          to discard values in the bottom or top of the stack.
* `push`: receives a string value as parameter and pushes it to the top of the stack.
* `pop`: receives an optional parameter with the number of values to pop from the stack, defaulting
         to 1. Once popped, the values are discarded. If this operation leaves the stack without
         values, it will fail.
* `dup`: receives an optional parameter with the index of a value to duplicate and push to the stack.
         If no value is specified, the topmost value is duplicated and pushed, so the two topmost
         values will be identical.
* `xchg`: Exchanges the topmost value for the specified parameter value. The topmost value is
          dropped. This operation is equivalent to a `pop` followed by a `push` without the risk of
          failing if the `pop` leaves the stack temporarily empty.
* `swap`: interchanges the values at the given positions in the stack. Receives two required
          parameters: `from` and `to`, accepting 0-based indexes in the stack.
* `indexes`: takes a list of indexes from the stack and drops the rest, resulting in a new stack. An
             empty list of indexes performs no operation. Useful if you know you need specific values
             only from the stack.
* `flat_map`: takes a list of operations and runs them all on each value in the stack, presenting a
              single-value stack to each run of the operations. Then it picks up the results and
              flattens them into a new stack. For example, `flat_map`ping the list of operations
              [`strrev`, `split`, `reverse`] on a stack like [`"abc:123"`, `"def:456"`] will result
              in the new stack [`"cba"`, `"321"`, `"fed"`, `"654"`]. The failure of any operation will
              mark this operation as failed.
* `select`: takes a list of operations and runs them all on each value in the stack, presenting a
            single-value stack to each run of the operations. It outputs a new stack in which values
            will be pushed to it if the operations were successful, and not pushed (and dropped) if
            they failed, disregarding any potential output in both cases. For example, you can select
            any values with a length over 5 by `select`ing with an operation like `len` with `min`
            set to `6`.
* `values`: logs the current stack values. It takes two optional parameters: `level`, which specifies
            the log level to use, defaulting to `info`, and `id`, used to add a tag to identify the
            log line and defaulting to an empty string. Log lines printed with this operation will
            have the string `"[3scale-auth/stack]"` printed before the `id` and values in the stack.
            _Note_ that the stack at some points might be different than what the whole stack of
            values due to how some operations present a smaller or different stack to other inner
            operations.

### Check operations

The operations in this category help you build checks on other operations. Just checking for
properties means these operations never have side effects: they discard any changes to the stack
once they have been resolved.

* `ok`: this operation takes no parameters and always succeeds.
* `fail`: this operation takes no parameters and always fails.
* `any`: this takes a list of operations as parameter and runs them all until one of them succeeds.
         The first successful operation marks this one as successful without further evaluation,
         that is, [`short-circuiting`](https://en.wikipedia.org/wiki/Short-circuit_evaluation).
* `one_of`: this takes a list of operations as parameter and runs them all ensuring only one of them
            is successful. If none or more than one operation succeeds, the operation is marked as
            failed, that is, this operations behaves like an [`XOR`](https://en.wikipedia.org/wiki/Exclusive_or).
* `all`: this takes a list of operations as parameter and runs them all until one of them fails,
         which would mark this operation as failed `short-circuiting` the rest.
* `none`: this takes a list of operations as parameter and runs them all until one of them succeeds,
          which would mark this operation as failed `short-circuiting` the rest.
* `assert`: this takes a list of operations which will run with a cloned stack, where having all
            the operations being successful makes this one successful as well but the output is
            _completely dropped and no change happens in the stack_. Note that this is _not_ a
            more concise version of the `test` operation below without an `else` parameter, as `test`
            _has side effects_ when taking any one of the branches.
* `refute`: this takes a list of operations which will run with a cloned stack, where having all
            the operations being a failure makes this one successful, with _no change happening in
            the stack_.

### Control operations

The operations in this category help you modify the control flow on other operations. These don't
allow for implementing arbitrary Turing-complete programs, but help in ensuring certain properties.
Contrary to `check` operations, these have side effects.

* `test`: this operation implements conditionals. It takes two mandatory parameters: `if`, which
          specifies an operation that acts as an assertion, that is, it will have no side effects,
          and `then`, which receives a list of operations. If the operation in `if` succeeds, the
          list of operations in `then` will be evaluated on the stack as it was before evaluating
          this `test` operation, and the outcome of that list of operations will become the
          outcome of this operation. Conversely, the optional parameter `else` specifies the list
          of operations to evaluate when the operation in `if` fails. If `else` is not specified
          or is empty, the list for this case is implicitly filled with a `true` operation.
* `and`: this operation evaluates any passed in operations in sequence, requiring all of them to
         succeed while having side effects. This is actually the default evaluating behavior when no
         evaluation modifying operation is being evaluated, but this is useful to logically group
         operations separate from others, and as a single operation composed of multiple operations
         with [`AND semantics`](https://en.wikipedia.org/wiki/Logical_conjunction).
* `or`: this operation evaluates any passed in operations in sequence giving them, separately, the
        current stack as input, and succeeding and stopping evaluation when any one of them succeeds
        while keeping its side effects.
* `xor`: this operation evaluates any passed in operations in sequence giving them, separately, the
         current stack as input, and succeeding only when any one, but only one, of them succeeds
         while keeping its side effects. This operation necessarily evaluates all operations.
* `cloned`: takes two parameters: `result`, which is optional, and `ops`, required. `ops` is a list of
            operations to run on a _clone_ of the current stack, and `result` specifies whether the
            output of such operations will be appended to the original stack (default if not specified)
            or prefixed to the original stack, so the accepted values for this parameter are `prepend`
            or `append`. For example, one does not need to drop the original value for a string `split`
            operation, but instead they can use `cloned` with `result: prepend` and `split` in the `ops`
            list so that an input stack with `"user:password"` would result in [`"user"`, `"password"`,
            `"user:password"`].
* `partial`: takes three parameters: `result` and `max`, optional, and `ops`, required. Similar to
             `cloned`, this operation will run a series of other operations and `prepend` or `append`
             the results depending on the value of `result` (default to `append`). What makes this a
             very different operation is that no cloning of values happens, but just a new stack
             containing up to `max` topmost values in the stack is presented to the specified
             operations - so they won't be able to affect the remainig values. `max` defaults to `1`
             (the topmost element). _Note_: a `max` value of `0` has undefined behavior.
* `top`: receives a list of operations that will be evaluated with a stack initially consisting of
         just the top element of the current stack, and the results of such operations, if
         successful, will be added to the previous stack. This operation is a subset of `partial`,
         which you can recreate with parameters `result: append` and `max: 1`.
* `log`: this operation logs a message to the module logs. It takes one optional `level` parameter
         defaulting to the `info` log level, and one mandatory `msg` parameter, with the string
         message to print in the logging system. Messages logged in this way will be preceded by
         the string `"[3scale-auth/config]"`.

### Format-specific operations

These `operation`s allow you to look up specific structure and fields within a container format.
All of the fields are `structured format` objects.

The following `format`-specific operations are currently supported:

* `json`: parses the input value as a `JSON` object.
* `protobuf`: parses the input value as a [`ProtoBuf`](https://en.wikipedia.org/wiki/Protocol_Buffers)
              [`Structure`](https://developers.google.com/protocol-buffers/docs/reference/google.protobuf#google.protobuf.Struct)

The `json` and `protobuf` objects describe operations on structured formats and they share the
same fields. See their definitions below.

#### The `structured format` objects

Currently there are two such objects accepting the exact same fields:

* `path`: Required. An array of strings describing a lookup path in the structured object. See below
          for semantics. Can be left empty to signal no look up is required on the input value.
* `keys`: Required. An array of strings listing the `keys` to try out in a `structured format` object
          after traversing the `path` above. This field can specify an empty array to signal the looked
          up value is not expected to support querying for entries or `keys`, ie. it is a string, a
          number, a boolean, or a list, but not an object, map or structure.

There are two phases to these operations. First, a `path` is resolved within the `structured format`.
If the `path` does not resolve to a string or a container value, that is, a list of values or a
structure or object mapping fields to values, then the operation fails.

Then a second phase where `keys` are probed in order taking the output of the `path` fase as input. In
this phase any one of the `keys`, when they are specified, should match the input and product an
output value or list of values to consider the operation successful.

**Note**: The paragraphs below describe how `path` and `keys` interact, but you might find looking at
examples easier to grasp, so if you feel the text is too dense, just skip it and check the examples.

If the `path` array contains no values, the input values is passed on as is to the second phase.
Otherwise the array specifies a single sequence of `components` or `segments` in the `path` that will
be looked up in succession by passing in the output of each `segment` look up to the next one within
the structure of the formatted value, so that the operation will end up obtaining a value that must
be one of a few cases only: a string, a list of values, or a structure object.

The `keys` field, when not empty, specifies strings that match entry names, list indexes, or string
values on its input, which is the output of a `path` look up. The `keys` are probed in order, and
when a key does not match, the next one is tried. Conversely, when a key matches, its corresponding
output value is taken as result and the resolution ends.

A successful second phase, including the cases where `keys` is left empty, can only output a string
or a list of strings. So if `keys` is left empty, the first phase no longer can output a structure
object, but only a string or a list of strings.

The way keys are interpreted and matched against the input depends on what the input looks like:

* If the input is a string, a key will match if it is equal to the input. The output is the matched
  string itself.
* If the input is a list of strings, a key will match if it can be parsed as a number indexing into
  the list (0-based), and the list contains at that index position a `resolvable output value`.
* If the input is a structure or object, a key will match if there is an entry with the name of the
  key in the structure, and its value is a `resolvable output value`.

A `resolvable output value` as described above is either a string, a list of strings, or a structure
object that has one single entry in which the associated value is either a string or a list of
strings. Anything that does not fall in these categories isn't such a value and won't succeed. An
example of something that is _not_ such a value is a list of strings that also contains a number.

This way the whole operation can only resolve to a string or a list of strings.

The following rules must be observed when specifying a `path` array of strings as they relate to
`structural equality`:

* All elements in the array are strings with differing meaning depending on the input value, and all
  of these elements are known as `path` `segments` or `components`.
* If a `segment` in the `path` is successfully looked up, the looked up value is used as input for
  the next `segment` look up.
* If any `segment` fails the look up, the `format-specific operation` fails.
* Structures, also known as object or maps, all have entries. When the input value is such an object,
  a `path` `segment` is meant to match a field name of such a structure, and the associated value
  is the look up output value, which will in turn be the input for the next `segment` look up.
* Whenever a structure, object or map has been looked up with a particular `path` `segment`, and that
  `segment` is a literal `0` that has failed a look up (because there is no "0" entry), _and the
  structure contains just one entry_, then such an entry will match, and its associated value will be
  the output of this `0` `segment` look up.
* Whenever a list or array of values is being matched, the `path` `segment` will be parsed as a
  number representing an index into the list, starting from `0`. The output of the look up will be
  the value in that position, if the index exists within the list.
* Whenever a string is being matched, the `path` `segment` will only match if it contains the same
  exact string. The output value is the matched string itself.
* Whenever an entry is matched and contains a value that is not a list, a structure, or a string, the
  look up will be considered failed. That is, there is no coercion from numbers or booleans to
  strings.
* A `path` lookup should always resolve to a string or list of strings, and only if `keys` is not
  empty it can, in addition, resolve to a structure or object. This is because the output of the
  `path` look up would become the operation's resolved value, which can only be a string of a list
  of strings, whereas the `keys` evaluation can have a structure as input and produce a string or
  a list of strings.

The main differences between `path` and `keys` phases are:

* `path` `segments` must all resolve in sequence. If a `segment` look up fails, there is no second
  try with another `segment` at the same level, and the operation fails. `keys` will just try with
  the next key in the list of `keys` until one successfully resolves.
* `keys` will not perform `path`-like deep look ups. `keys` are meant to match potential different
  entries in a structure, or different positions in a list, or even the contents of a string, but
  they won't go deeper in the structure other than when a match has a `resolvable output value`.
  When dealing with a `resolvable output value`, a reasonable best effort is performed to locate an
  unambiguous string or a list of strings, with the only special case being the one where a
  structure with a single entry having such an associated value.

#### Example 1

Say you look up a secret in a JSON object with a path like "claims", "my.domain", "secrets", which
returns, when existing, a structure with potentially two places where you'd find a secret, or a
list of secrets. Out of these two places, you prefer to first look up the one with a single secret.

This could be the input:

```json
{
  "claims": {
    "some.domain": {},
    "my.domain": {
      "some_data": [],
      "secrets": {
        "main_secret": "an_important_secret",
        "secondary_secrets": ["random_secret1", "random_secret2"]
      }
    }
  }
}
```

The following object would be relevant to pick up either the main secret, if present, or the secondaries:

```yaml
  json:
    path:
      - claims
      - my.domain
      - secrets
    keys:
      - main_secret
      - secondary_secrets
```

#### Example 2

You have a `JSON` object which has a single entry which has a custom name you can't know in advance.
The value of that entry should be a list of values for which the second entry is expected to be the
string we are looking for.

This could be the input:

```json
{
  "a7d12fba9c025e63": [
    {
      "name": "James"
    },
    "a_s3kr3t",
    {
      "address": {}
    }
  ]
}
```

The following object would be relevant to obtain the secret, if present:

```yaml
  json:
    path:
      - "0"
    keys:
      - "1"
```

*Note*: you should always specify `path`s and `keys` as strings rather than numbers or booleans.
