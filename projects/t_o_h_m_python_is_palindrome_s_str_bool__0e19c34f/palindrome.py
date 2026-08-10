def is_palindrome(s: str) -> bool:
    """
    Determine whether a string is a palindrome after normalization.

    Normalization folds the case of all characters and retains only Unicode
    alphanumeric characters (letters and digits).

    Parameters:
        s (str): The string to check.

    Returns:
        bool: True if the normalized string reads the same forwards and backwards,
              otherwise False.

    Raises:
        TypeError: If the input parameter is not a string.
    """
    if not isinstance(s, str):
        raise TypeError(f"is_palindrome() expected str, got {type(s).__name__}")

    normalized = "".join(ch.lower() for ch in s if ch.isalnum())
    return normalized == normalized[::-1]