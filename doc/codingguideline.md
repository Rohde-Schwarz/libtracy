# Libtracy Coding Guideline

This project uses the Kernighan & Ritchie coding style for the C part.
The Rust part uses (mostly) the same coding style the Rust standard library uses,
except for the brackets, which follow the K&R style.

Details for both cases are specified below. However, as this project is primarily
a Rust project, only a few rules for C are specified. If something is unclear
according C, stick to the K&R style.

## General Rules

### Spaces and Tabs

Use tabs to indent blocks. The tabwidth should be 4.

Always place spaces between binary operators and round brackets: `if (a && b) {`

No spaces after an unary operator: `if (!a) {`.

No spaces after a function name:

```
fn foo (...) // bad!
void foo (...) // bad!

fn foo(...) // good
void foo(...) // good
```

### Brackets

Brackets for functions have to be on the same vertical line:

Rust:

``` Rust
fn foo()
{
	// do_stuff
}
```

C:

```
void foo()
{
	/* do_stuff */
}
``` 

*All other brackets*, such as struct definitions and if-else branches, shall use
the style with the opening bracket on the same line as the construct:

``` Rust
if condition1 {

} else if condition2 {
``` 

### snake\_case and CamelCase

This project uses CamelCase for structures and enums and snake\_case for everything else.

### Variables

- Global Variables (called global statics in Rust) are started with an uppercase letter: `Environ`
- Local variables are always written all lowercase (with snake\_case): `environ\_one`
- Constants (and Rust statics) are written all uppercase: `TIME_US = 1000;`

## C specifics

### Enums

Enums in C shall be written in upper- or lowercase, depending on their scope,
as defined in the previous subsection.

The members of enums should always be written in capslock, to indicate that they
represent constant numbers.

``` C
/* global C enum */

enum My_enum {
	FOO;
	BAR;
};
```

``` C
/* local C enum */

enum my_enum {
	FOO;
	BAR;
};
```


## Rust specifics

### Enums

Rust enumeration types follow the usual Rust style and are written in CamelCase,
with their members being written in CamelCase as well.

``` Rust
enum TracerState {
    Normal,
    Terminate,
    DataProcessed,
}
```

### Methods

Methods are treated like functions and use `snake_case`.
