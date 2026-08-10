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
        # Unicode examples to verify Unicode-awareness
        "Añibila",  # ñ and b are letters (casefolded: añibila is palindrome)
        "123а321",  # Cyrillic 'а' inside numbers
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
def test_is_palindrome_type_error(bad: any) -> None:
    """Verify that non-string arguments raise a TypeError."""
    with pytest.raises(TypeError):
        is_palindrome(bad)
```