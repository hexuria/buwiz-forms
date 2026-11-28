"""An agent build of the Mac App Store package is never uploaded, and so never sent to review, by accident.

`just testflight-agent` builds with --agent: the driver's socket and com.apple.security.network.server. Such a build
marks itself (NSHAgentBuild in its Info.plist, agent=1 in its provenance); testflight-upload refuses it unless
ALLOW_AGENT_UPLOAD=1, which only testflight-agent sets, and lists an allowed one in agent-builds.txt so its build number
is never the one chosen for review (features/ios-store/CUT-1.0.0.md).
"""

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
JUSTFILE = (ROOT / "justfile").read_text(encoding="utf-8")


def recipe(name):
    match = re.search(rf"^{re.escape(name)}[ :].*?(?=^\S)", JUSTFILE, re.S | re.M)
    assert match, f"no recipe {name}"
    return match.group(0)


class AgentUploadGuardTests(unittest.TestCase):
    def test_only_an_agent_build_is_marked(self):
        body = recipe("_package-mac-appstore")
        mark = 'if [ "$NSH_MAS_AGENT" = "1" ]; then\n        /usr/libexec/PlistBuddy -c "Add :NSHAgentBuild bool true" "$PLIST"'
        self.assertIn(mark, body)
        # The mark goes in before the bundle is signed, so the signature covers it.
        self.assertLess(body.index("NSHAgentBuild"), body.index("codesign --force"))
        self.assertIn('build {{BUILD_NUMBER}}, agent=$NSH_MAS_AGENT" > "${PKG_PATH%.pkg}.provenance.txt"', body)

    def test_the_upload_refuses_an_agent_build_unless_allowed(self):
        body = recipe("testflight-upload")
        guard = body.index("AGENT_MARK=")
        self.assertLess(guard, body.index('"$TRANSPORTER" -m upload'))
        self.assertIn("Print :NSHAgentBuild", body)
        self.assertIn("grep -q 'agent=1'", body)
        self.assertIn('if [ "${ALLOW_AGENT_UPLOAD:-}" != "1" ]; then', body)
        refusal = body[guard:body.index('"$TRANSPORTER" -m upload')]
        self.assertIn("exit 1", refusal)
        self.assertIn('>> "{{RELEASE_DIR}}/agent-builds.txt"', refusal)

    def test_only_testflight_agent_allows_it(self):
        self.assertIn("ALLOW_AGENT_UPLOAD=1 just testflight --agent {{args}}", recipe("testflight-agent"))
        others = [line for line in JUSTFILE.splitlines() if "ALLOW_AGENT_UPLOAD=1" in line and not line.lstrip().startswith("#")]
        self.assertEqual(others, ["    ALLOW_AGENT_UPLOAD=1 just testflight --agent {{args}}"])


if __name__ == "__main__":
    unittest.main()
