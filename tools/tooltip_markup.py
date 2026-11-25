"""Apply the game's existing colour markup to reviewed, plain player copy.

Keep prose and numbers in player_tooltips.json; formatting never edits mechanics.
Tags use TFM2's <#RRGGBBAA>text<> syntax, as upstream Riot Items does.
"""
import re

COLOURS = {
    'name': 'ff7a3eff',
    'number': 'e8a800ff',
    'physical': 'd94c49ff',
    'magic': 'a974ffff',
    'true': 'ffffffff',
    'benefit': '92dc7bff',
    'range': 'ff86c2ff',
}

# One pass over plain text prevents nested spans and accidental tag colouring.
TOKENS = re.compile(
    r'(?P<name>^[A-Z][A-Za-z ’\'\-]+(?=:)(?<!Cooldown))'
    r'|(?P<range>attack range)'
    r'|(?P<physical>(?i:physical damage(?: immunity)?)|Physical Penetration)'
    r'|(?P<magic>(?i:magic damage(?: reduction)?)|Magic Power|Magic Penetration)'
    r'|(?P<true>true damage)'
    r'|(?P<benefit>Attack Speed|Movement Speed|Tenacity|stasis|untargetable|immune to damage|healing|shield(?:ing|s|ed)?)'
    r'|(?P<number>(?<![\w.])\+?\d+(?:\.\d+)?(?:[–−]\d+(?:\.\d+)?)?%?(?:s\b)?)',
    re.MULTILINE,
)


def colour_tooltip(text):
    if '<' in text or '>' in text:
        raise ValueError('Player copy must be plain text before formatting')
    return TOKENS.sub(lambda m: '<#' + COLOURS[m.lastgroup] + '>' + m[0] + '<>', text)


def plain_tooltip(text):
    return re.sub(r'<[^>]*>', '', text)
