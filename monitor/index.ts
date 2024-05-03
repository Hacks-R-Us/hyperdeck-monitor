import WebSocket from 'ws';

const wss = new WebSocket.Server({ port: 7867 });

wss.on('connection', function connection(ws) {
  ws.on('message', function message(data) {
    console.log('received: %s', data);
  });

  ws.send(JSON.stringify({
    event: "log",
    message: "Hello"
  }));
});
