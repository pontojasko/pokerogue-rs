// Optional manual preview. The automated runner uses Playwright routing, with no server.
import http from 'node:http';
import {localFile} from './local-files.mjs';
const server=http.createServer((req,res)=>{
 try {const file=localFile(new URL(req.url,'http://127.0.0.1'));res.setHeader('Content-Type',file.contentType);res.end(file.body);}
 catch {res.writeHead(404).end();}
});
server.listen(Number(process.argv[2]||0),'127.0.0.1',()=>console.log(server.address().port));
