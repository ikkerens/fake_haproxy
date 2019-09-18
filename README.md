# Fake haproxy [![](https://img.shields.io/crates/v/fake_haproxy.svg)](https://crates.io/crates/fake_haproxy) [![](https://github.com/ikkerens/fake_haproxy/workflows/Snapshot/badge.svg)](https://github.com/ikkerens/fake_haproxy/actions)

This is a simple tool that is capable of proxying both regular
and haproxy-v1 enabled connections towards another haproxy-v1 enabled server.

## Warning
#### This tool should never be exposed to the public web, it is solely intended for use inside firewalled networks.
This is because this tool is effectively capable of spoofing any IP address towards a haproxy enabled server.
All incoming connections should only come from a trusted source.

##### You have been warned.

## Installation & usage
##### If you have [Rust](https://rustup.rs/) installed:
```sh
cargo install fake_haproxy
fake_haproxy --forward :8080@proxy.enabled.server.com:80
```

##### If you have [Docker](https://www.docker.com/) installed:
```sh
docker run -p 8080:8080 ikkerens/fake_haproxy:0.1.0 ./app --forward :8080@proxy.enabled.server.com:80
```

##### If you use [Kubernetes](https://kubernetes.io/):
```yml
kind: Deployment
apiVersion: extensions/v1beta1
metadata:
  name: proxy
  labels:
    app: proxy
spec
  selector:
    matchLabels:
      app: proxy
  template:
    metadata:
      name: proxy
      labels:
        app: proxy
    spec:
      containers:
        - name: proxy
          image: 'ikkerens/fake_haproxy:0.1.0'
          command:
            - ./app
          args:
            - '--forward'
            - ':8080@proxy.enabled.server.com:80'
---
kind: Service
apiVersion: v1
metadata:
  name: proxy
  labels:
    app: proxy
  annotations:
    ingress.appscode.com/send-proxy: v1 # This annotation can be used when you use Voyager: https://appscode.com/products/voyager/
                                        # This will cause it to send the proxy header from your ingress
spec:
  ports:
    - name: proxy-forward
      protocol: TCP
      port: 80
      targetPort: 8080
  selector:
    app: proxy
```

##### Or, if you don't have any of these:
You can always find pre-compiled binaries on [our releases page](https://github.com/ikkerens/fake_haproxy/releases).