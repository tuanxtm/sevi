"""Freeze the launcher's exact font instances; keep every glyph and codepoint.

Developer tool only: requires fonttools 4.68.0. Cargo uses the checked-in results.
Original variable fonts are downloaded to the workspace's ignored target folder.
"""
from pathlib import Path
from urllib.request import urlopen
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont
import hashlib
import json

root=Path(__file__).resolve().parents[3]
assets=Path(__file__).resolve().parent
cache=root.joinpath('target/ui-fonts')
assert assets.is_relative_to(root)
cache.mkdir(parents=True,exist_ok=True)
revision='0027613a909edfaa80eefd0b4027168dd3d5c876'
base='https://raw.githubusercontent.com/storytold/craft-launcher/'+revision+'/assets/fonts/'
sources={
    'Inter':'Inter-Variable.ttf',
    'GeistMono':'GeistMono-Variable.ttf',
}
expected={'Inter':'29160a80ff49ddcab2c97711247e08b1fab27a484a329ce8b813d820dc559031',
          'GeistMono':'d00e590b8eb3a59acc329b2d044fd143ae935090b7da33199ebee27cc7de8196'}
variants=[('Inter','Inter.ttf',{'wght':400,'opsz':14}),
          ('Inter','Inter-Medium.ttf',{'wght':500,'opsz':14}),
          ('GeistMono','GeistMono.ttf',{'wght':400}),
          ('GeistMono','GeistMono-Medium.ttf',{'wght':500}),
          ('GeistMono','GeistMono-Bold.ttf',{'wght':700})]
report=[]
for family,name,coordinates in variants:
    source=cache.joinpath(sources[family])
    if not source.exists():source.write_bytes(urlopen(base+sources[family]).read())
    assert hashlib.sha256(source.read_bytes()).hexdigest()==expected[family]
    font=TTFont(source,recalcTimestamp=False);coverage=font.getBestCmap();glyphs=font.getGlyphOrder()
    axes={axis.axisTag:coordinates.get(axis.axisTag,axis.defaultValue) for axis in font['fvar'].axes}
    output=instantiateVariableFont(font,axes,inplace=False,optimize=True)
    output.recalcTimestamp=False
    assert output.getBestCmap()==coverage and output.getGlyphOrder()==glyphs
    destination=assets.joinpath(name)
    output.save(destination)
    loaded=TTFont(destination)
    assert loaded.getBestCmap()==coverage and 'fvar' not in loaded
    report.append({'file':name,'coordinates':axes,'bytes':destination.stat().st_size,
                   'codepoints':len(coverage),'glyphs':len(glyphs),
                   'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest()})
assets.joinpath('font-instances.json').write_text(json.dumps({'source_revision':revision,'faces':report},indent=2)+'\n')
print(json.dumps(report,indent=2))
