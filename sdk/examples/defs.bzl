"""Naming shared by the SDK examples of every language."""

def example_name(stem):
    """The target name of an example whose file stem is `stem`: `TextEditor` and `text_editor` both give `text_editor`.

    Args:
        stem: the example's file or directory name without its extension.

    Returns:
        The stem in snake_case.
    """
    name = ""
    for index, char in enumerate(stem.elems()):
        if char.isupper() and index > 0 and stem[index - 1] != "_":
            name += "_"
        name += char.lower()
    return name
