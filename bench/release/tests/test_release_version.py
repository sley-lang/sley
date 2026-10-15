"""A candidate uses its workspace identity and only its exact-tag authority."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT=Path(__file__).resolve().parents[3]
sys.path.insert(0,str(ROOT/'scripts'))
import release_version
import publication_authority
import build_release_candidate as packaging
import build_reproducibility_report as reproducibility


class ReleaseVersionTests(unittest.TestCase):
    def test_candidate_identity_comes_from_workspace(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            for version in ('2.0.6','2.0.7','2.1.0-rc.1'):
                (root/'Cargo.toml').write_text('[workspace.package]\nversion = '+json.dumps(version)+'\n')
                self.assertEqual(release_version.artifact_name(root),f'sley-{version}-linux-x86_64.tar.gz')
            for version in ('../wrong','2.0','02.0.7','2.0.7/other','2.0.7\n'):
                (root/'Cargo.toml').write_text('[workspace.package]\nversion = '+json.dumps(version)+'\n')
                with self.assertRaises(ValueError): release_version.workspace_version(root)

    def test_prior_release_authorization_does_not_cover_new_tag(self):
        summary={'publication_authorized':True,'publication_decision':{
            'state':'PUBLICATION_AUTHORIZED','authority':'operator','decided':'2026-10-04',
            'scope':['synthetic test only'],'authorized_tags':['v2.0.6'],'public_repository':'https://example.invalid/repo'}}
        self.assertTrue(publication_authority.authorized_for_tag('v2.0.6',summary))
        self.assertFalse(publication_authority.authorized_for_tag('v2.0.7',summary))
        self.assertFalse(publication_authority.authorized_for_tag('v2.0.60',summary))
        self.assertFalse(publication_authority.authorized_for_tag('v2.0.7',{'publication_authorized':False}))
        with self.assertRaises(ValueError): publication_authority.authorized_for_tag('',summary)
        with self.assertRaises(ValueError): publication_authority.authorized_for_tag('v2.0.7',{'publication_authorized':True})

    def test_staging_refuses_stale_workspace_inventory(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); binary=root/'sley'; binary.write_bytes(b'fixture')
            binary.with_name('sley-agent').write_bytes(b'fixture')
            inventory=copy.deepcopy(json.loads(packaging.INVENTORY.read_text()))
            next(p for p in inventory['packages'] if p.get('workspace') and p['ecosystem']=='cargo')['version']='0.0.0'
            path=root/'inventory.json'; path.write_text(json.dumps(inventory))
            with patch.object(packaging,'INVENTORY',path):
                with self.assertRaisesRegex(packaging.PackageError,'workspace versions differ'):
                    packaging.stage_artifact(binary,root/'stage',commit='a'*40,toolchain={},working_tree_clean=False,blockers=['test'])

    def test_builder_and_workspace_names_agree(self):
        self.assertEqual(packaging.ARTIFACT_NAME,release_version.artifact_name())
        self.assertIn('scripts/release_version.py',packaging.ARTIFACT_INPUT_PATHS)
        self.assertIn('scripts/publication_authority.py',packaging.ARTIFACT_INPUT_PATHS)

    def test_prior_release_evidence_cannot_qualify_current_version(self):
        report=json.loads((ROOT/'evidence/release/reproducibility-report.json').read_text())
        historical=dict(report['attestations'][0],artifact_name='sley-2.0.6-linux-x86_64.tar.gz')
        with patch.object(reproducibility,'ARTIFACT_NAME','sley-2.0.7-linux-x86_64.tar.gz'):
            self.assertFalse(reproducibility.admissible_attestation(historical))
            with self.assertRaisesRegex(reproducibility.ReproError,'artifact_name differs'):
                reproducibility.validate_attestation(historical)

    def test_reproducibility_authority_is_scoped_to_candidate_tag(self):
        report=json.loads((ROOT/'evidence/release/reproducibility-report.json').read_text())
        row=report['attestations'][0]
        summary={'publication_authorized':True,'publication_decision':{
            'state':'PUBLICATION_AUTHORIZED','authority':'operator','decided':'2026-10-04',
            'scope':['synthetic test only'],'authorized_tags':['v2.0.6'],'public_repository':'https://example.invalid/repo'}}
        for version,want in [('2.0.6',True),('2.0.7',False)]:
            name=f'sley-{version}-linux-x86_64.tar.gz'
            with patch.object(reproducibility,'ARTIFACT_NAME',name), patch.object(publication_authority,'load_summary',return_value=summary):
                # Synthetic evidence exercises the flag, never qualifies a real build.
                rebuilt=reproducibility.build_report([dict(row,artifact_name=name)])
                self.assertIs(rebuilt['publication_authorized'],want)


if __name__=='__main__': unittest.main()
