# infrastructure
This provides the infrastructure to support the development of dApps, improving transaction and event management

![Alt text](https://g.gravizo.com/svg?
  digraph {
    node [shape=box]
    N [label="Ethereum Node"]

    T [label="Tx Manager"]
    I [label="State Manager"]
    T -> N
    I -> N

    D [label="Dispatcher"]

     A [label="Dapp Callback"]
    C [label="Configuration file"]
    D -> {I T A C}
  }
)
