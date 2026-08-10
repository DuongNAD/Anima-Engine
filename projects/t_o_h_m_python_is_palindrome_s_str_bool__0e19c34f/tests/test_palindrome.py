import pytest
from palindrome import is_palindrome

@pytest.mark.parametrize('text', [
    'A man, a plan, a canal: Panama',
    'racecar',
    'RaceCar',
    'No lemon, no melon',
    'Was it a car or a cat I saw?',
    '12321',
    'a',
    'Aa',
    '',
    '   ',
    '.,!?'
])
def test_is_palindrome_true(text):
    assert is_palindrome(text) is True

@pytest.mark.parametrize('text', [
    'hello',
    'abc',
    'This is not one',
    '12345',
    '0P',
    'ab'
])
def test_is_palindrome_false(text):
    assert is_palindrome(text) is False

@pytest.mark.parametrize('bad', [
    None,
    123,
    3.14,
    ['a'],
    ('a',),
    True
])
def test_is_palindrome_type_error(bad):
    with pytest.raises(TypeError):
        is_palindrome(bad)