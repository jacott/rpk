# Matrix Section

This section gives the keyboard's individual switches a symbolic name. See the [firmware] section to
see how to configure the MCU pins into rows and columns. The matrix section identifier takes an
additional suffix which defines how many rows and columns are being mapped[^note1] like
`[matrix:4x12]` which would indicate 4 rows and 12 columns. Assignments in this section start with a
row-column id assigned to one or more symbols. Matrix is the only required section in a config file.

### Example

```ini
[matrix:4x3]

0x00 = 7 8 9
0x10 = 4 5 6
0x20 = 1 2 3
0x30 = 0 . -
```

The left-hand-side of the assignment is a matrix location (row, column) in hexidecimal---indicated
by the `0x` prefix---which is partitioned into rows and columns; `0x15` for example would indicate
row 1 column 5. If there are more than 16 rows or columns then four digit hex numbers can be used
like `0x0a13` for row 10 column 19. One does not need to define a whole row per assignment but only
one row can be assigned per line.

### Complex Example

```ini
[matrix:2x3]

0x00 = 7
0x01 = 8 9
0x10 = 4
0x12 = return
0x11 = k11
```

In the complex example the first row is defined in two assignments and the second row defines each
key separately. Note that `k11` is not a valid [keycode][1]---it will map to `noop` (No
operation)---but can be used instead of `0x11` in other parts of the config file. So `0x12`,
`0x0104`, `return` all refer the keyboard switch at row 1, column 2 which is mapped by default to
keycode `return`.

When a valid keycode is used to name a keyboard switch it will be assigned by default to the
[main](layers.md) layout.

# Aliases Section

You can give a matrix location additional names using the aliases section. The main use of this is
to give an additional name to multiple switches which will then allow assigning a action/keycode to
multiple switches with one assignment.

### Example

```ini
[matrix:3x3]

0x00 = a b c
0x10 = d e f
0x20 = g h i

[aliases]

g = hyper
i = hyper

[main]

hyper = overload(nav, space)

[nav]

b =       up
d = left down right
```

This example means that if `g` or `i` is tapped a `space` will be emitted and will
switch to the `nav` layer if held.

[1]: https://en.wikipedia.org/wiki/Keycode

---

[^note1]: The matrix suffix can be a bit redundant if the firmware section defining pins is present
    but since the firmware section is optional we always need to define it as part of the matrix
    section header.
