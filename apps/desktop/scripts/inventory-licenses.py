#!/usr/bin/env python3
"""Offline license evidence collection, not a legal-compliance certification."""
import argparse, hashlib, json, os, pathlib, re, subprocess
ROOT=pathlib.Path(__file__).resolve().parents[3]
ENV={k:v for k,v in os.environ.items() if not k.startswith('GIT_')}

def digest(data):return hashlib.sha256(data).hexdigest()
def supplemental_text(root,folder,kind,license_name,extra):
    vcs=json.loads((folder/'.cargo_vcs_info.json').read_text()) if kind=='cargo' else {}
    file=(root/'docs/licenses'/extra['file']).resolve()
    if (not file.is_relative_to((root/'docs/licenses/upstream').resolve()) or license_name!=extra['license']
        or vcs.get('git',{}).get('sha1')!=extra['revision']):
        raise ValueError('Supplemental license evidence no longer matches its exact package source')
    data=file.read_bytes();data.decode('utf-8')
    if digest(data)!=extra['sha256']:raise ValueError('Supplemental license text digest mismatch')
    return extra['file'],data
def license_files(folder,declared_file=None):
    folder=folder.resolve();candidates=[]
    if declared_file:candidates.append(folder/declared_file)
    # Includes nested license/notice directories, bounded depth and file sizes.
    if folder.is_dir():
        for directory,children,files in os.walk(folder,followlinks=False):
            relative=pathlib.Path(directory).relative_to(folder)
            if len(relative.parts)>=3:children[:]=[]
            else:children[:]=[c for c in sorted(children) if not c.startswith('.') and c.lower() in ('license','licenses','licences','notice','notices','third-party-licenses')]
            for name in sorted(files):
                if re.match(r'^(licen[cs]e|notice|copying|copyright)([._-]|$)',name,re.I):candidates.append(pathlib.Path(directory)/name)
    found=[]
    for file in sorted(set(candidates)):
        resolved=file.resolve()
        if not resolved.is_relative_to(folder) or not resolved.is_file():continue
        size=resolved.stat().st_size
        if size>1024*1024:raise ValueError('License text exceeds 1 MiB; review manually')
        data=resolved.read_bytes();data.decode('utf-8') # Preserve original bytes, never lossy replacement.
        found.append((str(file.relative_to(folder)),data))
        if len(found)>64:raise ValueError('Too many license files in one package; review manually')
    return found

def main():
    p=argparse.ArgumentParser();p.add_argument('--target',default='aarch64-apple-darwin');p.add_argument('--output',default=str(ROOT/'docs/licenses'));a=p.parse_args()
    if not re.fullmatch(r'[A-Za-z0-9_-]{1,100}',a.target):p.error('Invalid target')
    output=pathlib.Path(a.output).resolve();output.mkdir(parents=True,exist_ok=True)
    raw=subprocess.check_output(['cargo','metadata','--locked','--offline','--format-version','1','--filter-platform',a.target,'--features','gitnova-desktop/custom-protocol'],cwd=ROOT,env=ENV,timeout=120)
    cargo=json.loads(raw);members=set(cargo['workspace_members']);rows=[];texts={};unresolved=[];review=[]
    supplements=json.loads((ROOT/'docs/licenses/supplemental-evidence.json').read_text())
    if supplements.get('schemaVersion')!=1:raise ValueError('Unexpected supplemental evidence schema')
    def add(kind,name,version,license_name,folder,present,role,declared_file=None):
        ident=f'{kind}:{name}@{version}';found=license_files(folder,declared_file) if present else []
        extra=supplements['components'].get(ident)
        if extra and present:
            found.append(supplemental_text(ROOT,folder,kind,license_name,extra))
        files=[]
        for path,data in found:
            sha=digest(data);texts.setdefault(sha,{'data':data,'components':[]})['components'].append(f'{ident} / {path}')
            files.append({'path':path,'bytes':len(data),'sha256':sha})
            if extra and path==extra['file']:files[-1]['source']=extra['source']
        row={'id':ident,'ecosystem':kind,'name':name,'version':version,'declaredLicense':license_name,'sourcePresent':present,'classification':role,'licenseFiles':files}
        rows.append(row)
        if not present:unresolved.append({'component':ident,'reason':'Source not installed/cached for this target; license files not inspected.'})
        elif not license_name or not files:unresolved.append({'component':ident,'reason':'Missing license declaration or collected license text; manual review required.'})
        if license_name and ('MPL' in license_name or 'WITH ' in license_name or 'Unicode' in license_name):review.append({'component':ident,'reason':'Review component-specific terms, selected licensing and distributed material; declaration alone is not an audit.'})
    for package in sorted(cargo['packages'],key=lambda v:(v['name'],v['version'])):
        if package['id'] in members:continue
        folder=pathlib.Path(package['manifest_path']).parent
        add('cargo',package['name'],package['version'],package.get('license'),folder,folder.is_dir(),'resolved graph including runtime/build/development; actual binary contribution not classified',package.get('license_file'))
    lock=json.loads((ROOT/'apps/desktop/package-lock.json').read_text())
    for key,entry in sorted(lock['packages'].items()):
        if not key:continue
        if not key.startswith('node_modules/') or '..' in pathlib.PurePosixPath(key).parts:raise ValueError('Unsafe package-lock path')
        folder=ROOT/'apps/desktop'/key;installed={}
        if (folder/'package.json').is_file():installed=json.loads((folder/'package.json').read_text())
        name=installed.get('name') or key.rsplit('node_modules/',1)[-1]
        license_name=entry.get('license') or installed.get('license')
        if isinstance(license_name,dict):license_name=license_name.get('type')
        if not isinstance(license_name,str):license_name=None
        add('npm',name,entry.get('version','unknown'),license_name,folder,folder.is_dir(),'development/build candidate' if entry.get('dev') else 'production/optional candidate; bundle contribution not classified')
    # Asset evidence is generated separately and explicit; never infer image ownership.
    assets=[]
    for folder in ['apps/desktop/assets','apps/desktop/src-tauri/icons','docs/screenshots']:
        for file in sorted((ROOT/folder).glob('*')):
            if file.is_file():assets.append({'path':file.relative_to(ROOT).as_posix(),'bytes':file.stat().st_size,'sha256':digest(file.read_bytes()),'review':'Project asset; ownership/provenance review required before public distribution.'})
    rows.sort(key=lambda r:r['id']);unresolved.sort(key=lambda r:r['component']);review.sort(key=lambda r:r['component'])
    report={'schemaVersion':1,'kind':'gitnova-license-inventory','target':a.target,'features':['gitnova-desktop/custom-protocol'],'cargoLockSha256':digest((ROOT/'Cargo.lock').read_bytes()),'npmLockSha256':digest((ROOT/'apps/desktop/package-lock.json').read_bytes()),'auditComplete':False,'components':rows,'assets':assets,'unresolved':unresolved,'manualTermsReview':review,'limitations':['Offline source/lock evidence, not a shipped-component or legal-compliance certification.','Includes build/development candidates and installed optional dependencies, not only runtime code.','Uninstalled other-platform packages remain explicitly uninspected; no inferred license text.','License alternatives, native frameworks, source-availability obligations and asset ownership need review for the actual distributed artifact.']}
    (output/'dependency-inventory.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    parts=[b'GitNova third-party license evidence\n==================================\n\nCollected from the offline locked dependency graph and installed npm packages.\nIncludes build/development candidates, not a statement that all are shipped.\nThis is not a completed license audit; see dependency-inventory.json for gaps.\nGitNova remains alpha; no public release compliance is certified here.\n\n']
    for sha,entry in sorted(texts.items()):
        header='\n\nComponents / original relative paths:\n'+'\n'.join(sorted(entry['components']))+f'\nText SHA-256: {sha}\n'+'-'*72+'\n'
        parts.extend([header.encode(),entry['data'],b'\n'])
    (output/'third-party-license-evidence.txt').write_bytes(b''.join(parts))
    print(json.dumps({'components':len(rows),'collectedUniqueTexts':len(texts),'unresolved':len(unresolved),'manualTermsReview':len(review),'assets':len(assets),'auditComplete':False}))
if __name__=='__main__':main()
