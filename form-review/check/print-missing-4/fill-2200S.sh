#!/bin/bash
# Fills the audit 2200S return (10 fixed Schedule 1 rows, no Others rows). Usage: fill-2200S.sh <addr> <token>
A="--addr $1"; export GPUI_AGENT_TOKEN=$2
gpui-agent invoke profile.save $A --arg tin=00012345300000 --arg tin_root=000123453 --arg name="Juana Santos" --arg taxpayer_type=individual --arg rdo_code=008 --arg zip_code=1004 --arg registered_address="1234 Socorro St" --arg phone=09171234567 --arg email=juana@example.com >/dev/null 2>&1 || true
gpui-agent invoke profile.forms_set $A --arg tin=00012345300000 --arg year=2026 --arg confirm=true --arg 'codes=["2200A","2200S","2550Q","1702Q","0605","1701","1600WP","1702MX"]' >/dev/null 2>&1
gpui-agent invoke form.save_draft $A >/dev/null 2>&1
gpui-agent invoke form.dismiss $A >/dev/null 2>&1
gpui-agent invoke form.open $A --arg code=2200S --arg year=2026 --arg period=9 --arg tin=00012345300000 >/dev/null
gpui-agent invoke form.fill $A --arg "fields=$(cat /tmp/p4s-fixed.json)" >/dev/null
gpui-agent invoke form.save_draft $A >/dev/null && echo SAVED-2200S
