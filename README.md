# Bungeespoof Proxy 2

Based on [serverscanning/bungeespoof-proxy](https://github.com/serverscanning/bungeespoof-proxy)

Allows you to connect to a bungeecoord backend server without using the intended bungeecoord proxy

I made this because the original bungeespoof proxy didn't work correctly when connecting to backend servers on versions other than 1.19.4, and it had an unnecessarily large number of unnecessary dependencies making compile times extremely long. I ended up deciding that it would be easier to re-implement the whole thing from the ground up instead of modifying the original.

---

## Usage

```
bungeespoof-proxy-2 [OPTIONS] --hostname <HOSTNAME> --username <USERNAME>

Options:
  -d, --hostname <HOSTNAME>            Host to connect to
  -l, --listen <LISTEN>                Where to listen for connections [default: 0.0.0.0:25570]
  -u, --username <USERNAME>            Username to log-in as
  -i, --uuid <UUID>                    UUID to log-in as [default: from-username]
  -n, --send-hostname <SEND_HOSTNAME>  Hostname to send to server [default: 0.0.0.0]
      --client-ip <CLIENT_IP>          Client IP to send to server [default: 192.168.0.1]
  -h, --help                           Print help
  -V, --version                        Print version
```

---

Thanks to [mat](https://github.com/mat-1/) who allowed me to use some of their code for handling DNS resolving and varints, and [Honbra](https://github.com/HonbraDev) for making the original bungeespoof proxy which this project was heavily inspired by.
