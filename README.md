# dicrs

![Alt text](screenshot.png?raw=true "dicrs")

### Controls

Shortcut | Action |
--- | --- |
`Ctrl-C` | Close the app |
`Ctrl-Y` | Copy a definition to the clipboard |
`Left/Right` | Move one dictionary up or down |
`Up/Down` | Move one word up/down in the index |
`Up/Down` + `Shift` | Move ten words up/down in the index |

### Dictionaries

Dictionaries are SQLite database files with the .db extension. Dictionaries contain two tables
1. A `name` table with a single `dicname` text column whose first entry contains the name of the dictionary (currently not used)
2. A `dictionary` table with two text columns `word` and `definition`. The `word` column contains words from which the index will be built. The `definition` columns contains the corresponding definition(s).

The dictionaries are stored in the `dics/` folder. The name of the dictionary's file will be the name displayed in the app. To rename or remove a dictionary, simply rename or remove the corresponding file.
