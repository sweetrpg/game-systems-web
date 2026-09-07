#!/usr/bin/env bash
# Fails the build if a template contains literal user-facing text that isn't
# sourced from a locale resource via a `tr.*()` lookup. See the
# `web-frontend-localization` spec (sweetrpg/platform,
# openspec/changes/full-localization-web-apps) for the convention.
#
# Allowed literals: brand/proper nouns and non-translatable strings, listed in
# ALLOWED below. Everything else between HTML tags must come from `{{ tr.* }}`.
set -euo pipefail
cd "$(dirname "$0")/.."

ALLOWED='SweetRPG|GitHub|Pilgrimage Software|^Sweet$|^RPG$|^v$|^&middot;$|^·$'
status=0

for template in templates/*.html; do
    # Strip askama blocks/expressions, then report any remaining text between tags.
    violations=$(
        perl -0777 -pe 's/<script.*?<\/script>//gs; s/\{%.*?%\}//gs; s/\{\{.*?\}\}//gs' "$template" |
            perl -0777 -ne 'while (/>[^<]+</g) { my $t = $&; $t =~ s/^>|<$//g; print "$t\n" if $t =~ /\S/; }' |
            grep -vE "(${ALLOWED})" || true
    )
    if [ -n "$violations" ]; then
        echo "ERROR: $template contains hardcoded user-facing strings:"
        echo "$violations"
        status=1
    fi
done

exit $status
