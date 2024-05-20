import { Hyperdeck, Commands } from 'hyperdeck-connection';
import WebSocket from 'ws';

interface WrappedHyperdeck {
  ip: String,
  port: number,
  hyperdeck: Hyperdeck
}

const hyperdecks: Map<string, WrappedHyperdeck> = new Map()

enum WebSocketMessageType {
  AddHyperdeck = "add_hyperdeck"
}

type WebSocketMessage = {
  type: WebSocketMessageType.AddHyperdeck,
  id: string,
  ip: string,
  port: number
}

const wss = new WebSocket.Server({ port: 7867 });

wss.on('connection', function connection(ws) {
  ws.on('message', function message(data) {
    try {
      const message = JSON.parse(data.toString()) as Partial<WebSocketMessage>;
      handle_message(message)
    } catch (_err) {
      return;
    }
  });

  ws.send(JSON.stringify({
    event: "log",
    message: "Hello"
  }));
});

function exhaustiveMatch(_never: never) {
  return;
}

function handle_message(message: Partial<WebSocketMessage>) {
  console.log(JSON.stringify(message));
  if (message.type === undefined) return;

  switch (message.type) {
    case WebSocketMessageType.AddHyperdeck:
      if (message.id === undefined) return;
      if (message.ip === undefined) return;
      if (message.port === undefined) return;
      if (isNaN(message.port)) return;
      if (message.port <= 0) return;

      console.log("Adding hyperdeck");

      const newHyperdeck = new Hyperdeck()

      // hyperdecks.set(message.id, {
      //   ip: message.ip,
      //   port: message.port,
      //   hyperdeck: newHyperdeck
      // });

      newHyperdeck.on('connected', (info) => {
        console.log(JSON.stringify(info))
      
        newHyperdeck.sendCommand(new Commands.TransportInfoCommand()).then((transportInfo) => {
          console.log(JSON.stringify(transportInfo))
        })
      })
      
      newHyperdeck.on('notify.slot', function (state) {
        console.log(JSON.stringify(state)) // catch the slot state change.
      })
      newHyperdeck.on('notify.transport', function (state) {
        console.log(JSON.stringify(state)) // catch the transport state change.
      })
      newHyperdeck.on('error', (err) => {
        console.log('Hyperdeck error', JSON.stringify(err))
      })

      newHyperdeck.connect(message.ip, message.port)

      break;
    default:
      exhaustiveMatch(message.type)
  }
}
