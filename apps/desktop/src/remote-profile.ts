import type {Connection} from './connection-store';
// The connection is a host, not a repository. Never guess owner/project from
// the local folder name or move credentials from Keychain into a clone URL.
export function profileRepositoryUrl(connection:Connection,path:string):string {
 const project=path.trim().replace(/\.git$/,'');
 if(!project||project.length>2048||project.split('/').length<2||project.split('/').some(part=>!part||part==='.'||part==='..'||! /^[A-Za-z0-9_.-]+$/.test(part)))throw new Error('Enter the owner/group and repository path, such as team/project.');
 return `${connection.origin}/${project}.git`;
}
