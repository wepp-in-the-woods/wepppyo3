"""Disposable NFS publication primitives; never touches run fixtures."""
import fcntl
import json
import os
from pathlib import Path
import tempfile

os.umask(0o022)
results={'uid':os.getuid(),'gid':os.getgid(),'root':'/wc1','operations':{}}
with tempfile.TemporaryDirectory(prefix='ashpost-publication-investigation-',dir='/wc1') as tmp:
    root=Path(tmp)
    with open(root/'lock','a+') as file:
        try:
            fcntl.flock(file,fcntl.LOCK_EX|fcntl.LOCK_NB)
            results['operations']['regular_file_flock']='passed'
            fcntl.flock(file,fcntl.LOCK_UN)
        except OSError as e:
            results['operations']['regular_file_flock']={'errno':e.errno,'error':str(e)}
    fd=os.open(root,os.O_RDONLY|os.O_DIRECTORY)
    try:
        try:
            fcntl.flock(fd,fcntl.LOCK_EX|fcntl.LOCK_NB)
            results['operations']['directory_flock']='passed'
            fcntl.flock(fd,fcntl.LOCK_UN)
        except OSError as e:
            results['operations']['directory_flock']={'errno':e.errno,'error':str(e)}
    finally:
        os.close(fd)
    post=root/'post';post.mkdir()
    for name in ['a','b']:
        (post/name).write_text('old');os.chmod(post/name,0o640)
        os.link(post/name,post/(name+'.backup'))
        (post/(name+'.stage')).write_text('new');os.chmod(post/(name+'.stage'),0o640)
    results['operations']['hardlink_backups']='passed'
    os.replace(post/'a.stage',post/'a')
    results['operations']['intermediate_visible_set']=[(post/n).read_text() for n in ['a','b']]
    os.replace(post/'a.backup',post/'a')
    results['operations']['rollback_visible_set']=[(post/n).read_text() for n in ['a','b']]
    results['operations']['restored_mode']=oct((post/'a').stat().st_mode & 0o777)
    stage=root/'stage';stage.mkdir();(stage/'value').write_text('complete')
    os.rename(stage,root/'first_publish')
    results['operations']['rename_directory_to_absent']='passed'
    stage=root/'stage';stage.mkdir();(stage/'value').write_text('complete')
    empty=root/'empty';empty.mkdir();os.rename(stage,empty)
    results['operations']['rename_directory_over_empty']='passed'
    (root/'current').symlink_to(post,target_is_directory=True)
    (root/'next').symlink_to(empty,target_is_directory=True)
    # Two independent opens across a flip do not pin a request-wide snapshot.
    first=(root/'current'/'b').read_text()
    os.replace(root/'next',root/'current')
    second=(root/'current'/'value').read_text()
    results['operations']['symlink_separate_reads_across_flip']=[first,second]
results['probe_directory_removed']=not root.exists()
print(json.dumps(results,indent=2))
