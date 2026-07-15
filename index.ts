import { Hyperdeck, Commands } from 'hyperdeck-connection';
import WebSocket, { WebSocketServer } from 'ws';
import { v4 as uuidv4 } from 'uuid';
import yargs from 'yargs';

interface WrappedHyperdeck {
  ip: String,
  port: number,
  hyperdeck: Hyperdeck
}

const hyperdecks: Map<string, WrappedHyperdeck> = new Map()

enum WebSocketMessageType {
  AddHyperdeck = "add_hyperdeck",
  RemoveHyperdeck = "remove_hyperdeck",
  StartRecording = "start_recording",
  StopRecording = "stop_recording"
}

type WebSocketMessage = {
  type: WebSocketMessageType.AddHyperdeck,
  id: string,
  ip: string,
  port: number
} | {
  type: WebSocketMessageType.RemoveHyperdeck,
  id: string
} | {
  type: WebSocketMessageType.StartRecording,
  id: string
} | {
  type: WebSocketMessageType.StopRecording,
  id: string
}

const args = await yargs(process.argv.slice(2))
  .usage('Usage: $0 [options]')
  .alias('p', 'port')
  .nargs('port', 1)
  .describe('port', 'Port to serve websocket on')
  .demandOption(['port'])
  .help('h')
  .alias('h', 'help')
  .parse();

const wss = new WebSocketServer({ port: args.port as number });
console.log(`Serving on port ${args.port}`);

const connected_clients: Map<string, WebSocket> = new Map();

wss.on('connection', (ws) => {
  const clientId = uuidv4();
  ws.on('message', (data) => {
    try {
      const message = JSON.parse(data.toString()) as Partial<WebSocketMessage>;
      handle_message(message)
    } catch (_err) {
      return;
    }
  });
  ws.on('close', () => {
    connected_clients.delete(clientId)
  })

  ws.send(JSON.stringify({
    event: "log",
    message: "Hello"
  }));

  connected_clients.set(clientId, ws);
});

function exhaustiveMatch(_never: never) {
  return;
}

function handle_message(message: Partial<WebSocketMessage>) {
  console.log(JSON.stringify(message));
  const messageType = message.type;
  if (messageType === undefined) return;

  switch (messageType) {
    case WebSocketMessageType.AddHyperdeck:
      if (message.id === undefined) return;
      if (message.ip === undefined) return;
      if (message.port === undefined) return;
      if (isNaN(message.port)) return;
      if (message.port <= 0) return;

      console.log("Adding hyperdeck");

      const newHyperdeck = new Hyperdeck()

      hyperdecks.set(message.id, {
        ip: message.ip,
        port: message.port,
        hyperdeck: newHyperdeck
      });

      newHyperdeck.on('connected', (_info) => {
        notifyClients({
          event: "hyperdeck_connected",
          id: message.id
        })
      
        setInterval(() => {
          newHyperdeck.sendCommand(new Commands.TransportInfoCommand()).then((transportInfo) => {
            notifyClients({
              event: "record_state",
              hyperdeck_id: message.id,
              status: transportInfo.status,
            })
          }).catch((err) => {
            console.log(JSON.stringify(err))
            notifyClients({
              event: "log",
              message: JSON.stringify(err)
            })
          })
        }, 1000)

        setTimeout(() => {
          newHyperdeck.sendCommand(new Commands.DeviceInfoCommand()).then((info) => {
            notifyClients({
              event: "log",
              message: "DEVICE INFO " + JSON.stringify(info)
            })
            let slots = info.slots === null ? 0 : info.slots;
            for (let index = 0; index < slots; index++) {
              setInterval(() => {
                newHyperdeck.sendCommand(new Commands.SlotInfoCommand(index)).then((slot) => {
                  notifyClients({
                    event: "log",
                    message: "SLOT " + JSON.stringify(slot)
                  })
                  notifyClients({
                    event: "record_time_remaining",
                    hyperdeck_id: message.id,
                    slot_id: slot.slotId,
                    remaining: slot.recordingTime
                  })
                }).catch((err) => {
                  console.log(JSON.stringify(err))
                  notifyClients({
                    event: "log",
                    message: "ERR " + JSON.stringify(err)
                  })
                })
              }, 1000)
            }
          })
          .catch((err) => {
            console.log(JSON.stringify(err))
            notifyClients({
              event: "log",
              message: "ERR " + JSON.stringify(err)
            })
          })
        }, 1000)
      })
      
      newHyperdeck.on('notify.slot', function (slot) {
        notifyClients({
          event: "record_time_remaining",
          hyperdeck_id: message.id,
          slot_id: slot.slotId,
          remaining: slot.recordingTime
        })
      })
      newHyperdeck.on('notify.transport', function (state) {
        notifyClients({
          event: "record_state",
          hyperdeck_id: message.id,
          status: state.status
        })
      })
      newHyperdeck.on('error', (err) => {
        console.log('Hyperdeck error', JSON.stringify(err))
        notifyClients({
          event: "log",
          message: JSON.stringify(err)
        })
      })

      newHyperdeck.on('disconnected', () => {
        notifyClients({
          event: "hyperdeck_disconnected",
          id: message.id
        })
      })

      newHyperdeck.connect(message.ip, message.port)

      break;
    case WebSocketMessageType.RemoveHyperdeck: {
      if (message.id === undefined) return;

      console.log("Removing hyperdeck");

      let hyperdeck = hyperdecks.get(message.id)
      if (hyperdeck === undefined) return;

      hyperdeck.hyperdeck.disconnect()
      hyperdecks.delete(message.id)

      break;
    }
    case WebSocketMessageType.StartRecording: {
      if (message.id === undefined) return;

      console.log("Starting recording");

      let hyperdeck = hyperdecks.get(message.id)
      if (hyperdeck === undefined) return;

      hyperdeck.hyperdeck.sendCommand(new Commands.RecordCommand());
      break;
    }
    case WebSocketMessageType.StopRecording: {
      if (message.id === undefined) return;

      console.log("Starting recording");

      let hyperdeck = hyperdecks.get(message.id)
      if (hyperdeck === undefined) return;

      hyperdeck.hyperdeck.sendCommand(new Commands.StopCommand());
      break;
    }
    default:
      exhaustiveMatch(messageType)
  }
}

function notifyClients(message: object) {
  connected_clients.forEach((client) => {
    client.send(JSON.stringify(message))
  })
}
