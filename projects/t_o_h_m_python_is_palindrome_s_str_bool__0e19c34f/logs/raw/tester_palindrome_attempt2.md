```python
import pytest
from palindrome import is_palindrome

@pytest.mark.parametrize(
    "text",
    [
        "A man, a plan, a canal: Panama",
        "racecar",
        "RaceCar",
        "No lemon, no melon",
        "Was it a car or a cat I saw?",
        "12321",
        "a",
        "Aa",
        "",
        "   ",
        ".,!?",
        # Unicode palindromes to verify Unicode-awareness
        "Añiña",  # ñ is a letter (casefolded: añiña is palindrome)
        "123а321",  # Cyrillic 'а' inside numbers
        "А роза упала на лапу Азора",  # Classic Russian palindrome
    ],
)
def test_is_palindrome_true(text: str) -> None:
    """Validate strings that should normalize to palindromes and return True."""
    assert is_palindrome(text) is True


@pytest.mark.parametrize(
    "text",
    [
        "hello",
        "abc",
        "This is not one",
        "12345",
        "0P",
        "ab",
        # 'Añibila' is not a palindrome ('añibila' reversed is 'alibiña', 'ñ' != 'l')
        "Añibila",
        # Unicode non-palindrome example
        "Привет",  # 'привет' -> 'тевирп' (not equal)
    ],
)
def test_is_palindrome_false(text: str) -> None:
    """Validate strings that should normalize to non-palindromes and return False."""
    assert is_palindrome(text) is False


@pytest.mark.parametrize(
    "bad",
    [
        None,
        123,
        3.14,
        ["a"],
        ("a",),
        {"a": "b"},
        True,
        False,
    ],
)
def test_is_palindrome_type_error(bad) -> None:
    """Verify that non-string arguments raise a TypeError with the correct message."""
    expected_msg = f"is_palindrome() expected str, got {type(bad).__name__}"
    with pytest.raises(TypeError) as exc_info:
        is_palindrome(bad)
    assert str(exc_info.value) == expected_msg
```