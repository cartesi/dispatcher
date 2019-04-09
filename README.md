# Installation

COMPLETE THESE INSTRUCTIONS

## Compile protoc

Instructions can be found here

    https://github.com/protocolbuffers/protobuf/blob/master/src/README.md

Install requirementes

    sudo apt-get install autoconf automake libtool curl make g++ unzip

Then compile

    git clone https://github.com/protocolbuffers/protobuf.git
    cd protobuf
    git submodule update --init --recursive
    ./autogen.sh

Then generate `protoc`

    ./configure
    make
    make check
    sudo make install
    sudo ldconfig # refresh shared library cache.

## Compile hasher and emulator_interface

Install requirements::

    sudo apt-get install gcc libssl-dev pkg-config

Install `protoc`.

Enter the hasher folder and enter

    cargo run --bin build-hasher

Enter the emulator_interface folder and enter

    cargo run

## Install further dependencies

    sudo apt-get install libssl-dev

## Compile

In the root folder

    cargo build

# TODOs

- change this file to reflect changes in the design
- improve error reporting by: adding more chain_err and inserting context inside the error messages
- implement display for the structs we define and use them in logs

# Infrastructure

This provides the infrastructure to support the development of dApps.
Dapps that follow these guidelines and use our offered infrastructure will be safer, more robust and easier to develop.

## Goals

The main goals of our design are in the following order:

- To assure *safety*. Among other things against: power failures, attacks, lost transactions.
- *Easy development*. DApps that use our infrastructure and adhere to our designs should have to worry much less about blockchain idiosyncrasies, no concerns with various instances running in parallel, no async programming, no worries about branching histories in the main chain.

To achieve these goals we will minimize the state that is kept by the application.
And whenever some state is unavoidable, we will build a safe and ergonomic interface around this state for the dApp developer.

These high-level goals also require a modular architecture, so that we can encapsulate the state inside modules with clear interfaces, thus approaching a functional design.
Modularity is also beneficial as it facilitates development and tests.

## Taming the state

Concerning the state, we have the following preferences (in decreasing order):

- no state at all,
- cache-only state (that can be reconstructed from other states),
- append-only state (no need of version control)

Moreover, when we have an unavoidable state, we should keep it encapsulated inside a single component that "owns that state".

Here are the states which are strictly necessary for Cartesi to work:

- The content of the blockchain (although we may cache some of it).
- The transaction pool (this is an annoying bit and only one module should know of its existence).
- A collection of files with their hashes.
In a first moment, this collection could be "append only", meaning that one can only add and retrieve files to the collection (garbage collecting them only when they are *provably* unnecessary).
Files need strong backup systems.

Every other state in the system should be able to bootstrap from the above ones.

## Overall Structure

The purpose of each of these components is briefly described below.
Later we will give an overview of the Dispatcher Loop, then a more detailed description of each module.

<img src="components.png" alt="drawing" width="500"/>

### Dispatcher

This component contains the main loop of the application, observing the state of the blockchain and acting on the behalf of the dApp.
This component is very central in the sense that it communicates to various others in the system.
This apparent complexity is mitigated by the fact that it contains no state and can always gracefully recover from a power-down.


### Transaction Manager

Whenever the dApp needs to send a transaction to the blockchain, it has the possibility to do it through our Transaction Manager, which will take care of all the bureaucracy from both the Ethereum Blochchain and the Transaction Pool, such as:

- signing transactions,
- lost transactions (for instance if the user increases its nonce in some other way),
- gas price optimization,
- reaching deadlines for the transactions to be mined.

### State Manager

DApps written for Cartesi will work best if their smart contracts have some "getter functions" that are predefined by us, guiding somehow the inner workings of the contract.
Since these getter functions are standard, we have a component dedicated to retrieve these data, which abstracts away:

- the specific blockchain that we are dealing with and
- whether we are working with a full or a light node.

### DApp Callback

This is where the dApp-specific action takes place.
In order to make a Cartesi dApp, a developer has to implement three things: 

- a few smart contracts,
- some machines to run in our emulator and
- the "DApp Callback".

This module is called by the Dispatcher, with all the state information that it needs in order to make decisions.
The Callback can also access files through the File Manager and finally return some action to the Dispatcher in the form of a transaction.

### Configuration File

This is a simple file holding data that is specific of the user's installation of the dApp, like: the addresses of the relevant smart contracts and the user's address.

### File manager

Cartesi offers the possibility to deal with large amounts of data on the blockchain.
This will only be possible because we can store large files off-chain, while refering to them on-chain through their Merkle-tree hashes.
The File Manager service will keep files on behalf of the dApp, reducing and encapsulating the state of the DApp Callback.
This manager can provide several aditional services in the future, such as backups, p2p sharing, making data available and collecting signatures, etc.

### Ethereum Node

This is self explanatory. Async, reasonably complex, but self explanatory.

## The Dispatcher Loop

We now start a more detailed explanation of each of the above components, starting with the main loop that happens inside the Dispatcher.

The Dispatcher should be stateless, so that in the event of a power-down, it can recover seamlessly.
Imagining that the Dispatcher just woke up, it will perform the following steps in order:

<img src="dispatcher.png" alt="drawing" width="500"/>

1. Open the Configuration File to collect all data that is specific to our user.
The main content of this file is the list of "concerns" as explained below.
1. Contact the State Manager to obtain all the blockchain information that is relevant to the user. Example: what is currently going on in the "partition contract", instance 17.
1. Ask the Transaction Manager which of these instances is being treated now and which needs a reaction from us.
1. All this information is passed to the DApp Callback, which will have to take a decision on how to proceed (more details later).
The DApp Callback then returns to the Dispatcher one (or more) transactions that should be sent to the blockchain.
1. The Dispatcher sends these transactions to the Transaction Manager that will make sure they are inserted into the blockchain in a timely manner.

## Configuration File

The configuration file uses the `yaml` language to store its data.
The most important element in this file is called `concerns` and it is a list of things that the dApp cares about.
Each element of the `concerns` list is a dictionary describing its various parameters.

An example of such file would be the following:

    # This is a config file for a dApp
    # starting with the list of concerns
    concerns:
      -
        contract: 0xf778b86fa74e846c4f0a1fbd1335fe81c00a0c91
        address: 0x6ac7ea33f8831ea9dcc53393aaa88b25a785dbf0
      -
        contract: 0xfffd933a0bc612844eaf0c6fe3e5b8e9b6c1d19c
        address: 0x6ac7ea33f8831ea9dcc53393aaa88b25a785dbf0

## Conformant Contracts

Each contract that wants to benefit from Cartesi's infrastructure must be organized in a certain fashion to facilitate interactions with it.

- `instances` The first aspect in this specification is that the smart contract should be organized around "instances".
More precisely, each conformant smart contract needs to contain an array of instances, each of which contains a struct describing that whole instance.
For example, in the partition smart contract, there is an array of instances, each of these instances represents a dispute between two players to find the point they disagree with.
- `uint currentInstance()` This returns the number of the last used instance plus one (so that it coincides with the number of instances already initialized).
- `bool isActive(uint instance)` These contracts should also implement a pure function, answering whether a given instance is still active or not.
This helps trim out the instances that need no more attention.
- `bool isConcerned(uint instance, address player)` There should be a pure function to determine if a certain player should be concerned with a certain instance.
Typically, this can be achieved by simply storing a list of concerned users and checking against it.
- `uint getNonce(uint instance)` Each instance should have a nonce that is incremented in every transaction (that is not reversed, of course).
This nonce is important in various moments for the off-chain component to decide on how to react.
- `bytes getState(uint instance)` This pure function returns the current state of one particular instance.
Note that all data which is necessary for players to react to this instance should be returned by this function, although not necessarily the full state of the contract.
For example, in the case of partition, one possible return for this function would contain something like `[nonce: 5, state: 2, queryArray: [0, 200, 400, 600]]` encoded appropriately.

## State Manager

The responsibilities of the State Manager are:

- query conformant contracts to find instances that are concerning to our users.
These queries should also include the `nonce` each instance,
- get the current state of the contract,
- cache this information for faster retrieval and
- do all of the above work efficiently with both a full or a light Ethereum node.

The state of this module can be fully bootstrapped from the information present in the blokchain, but caching is essential to get any acceptable performance.

#### Calls

The first query that the SM has a pair (contract, address) as input:

    { contract: 0xf778b86fa74e846c4f0a1fbd1335fe81c00a0c91,
      address: 0x6ac7ea33f8831ea9dcc53393aaa88b25a785dbf0
    }

It should return a list of active and concerning instances:

    [{ instance: 23, nonce: 22 }
     { instance: 218, nonce: 1 }]

The second type of queries that the SM receives inspects the state of a given instance:

    { contract: 0xf778b86fa74e846c4f0a1fbd1335fe81c00a0c91,
      instance: 23
    }

The response of this query gives simply the result the `getState` call:

    { nonce: 22,
      state: 2,
      queryArray: [0, 200, 400, 600]
    }

where the above could be encoded into a binary according to the Ethereum ABI.

#### State

The State Manager is capable of bootstrapping from zero in case of disk failure, so there is no need for a strong backup of its permanent data.

However, it is important for the State Manager to cache some information in order to avoid the long process of querying all data to the blockchain at each time.

This permanent data is a key-value table, where each key is of the form `(contract, address)` (decide a proper format):

    { contract: 0xf778b86fa74e846c4f0a1fbd1335fe81c00a0c91,
      address: 0x6ac7ea33f8831ea9dcc53393aaa88b25a785dbf0
    }

And for each such key, it should store something like:

    { last_current_instance: 344,
      last_block: 54320939,
      active_instances: [
        { instance: 23, nonce: 22 },
        { instance: 218, nonce: 1 }
      ]
    }

where `last_instance` corresponds to the last value that was received by the function `currentInstance()`.
If this value is not up to date with the blockchain, we have to inspect all the new instances (that have appeared in the meantime) and update our database accordingly.

#### Second implementation

Because of issues with the transaction pool, it may be important for the dApp to know both the `nonce` and the `user_nonce`, meaning: the last `nonce` of the instance that corresponds to a transaction by that user.

    { last_current_instance: 344,
      last_block: 54320939,
      active_instances: [
        { instance: 23, last_nonce: 22, last_user_nonce: 21 },
        { instance: 218, last_nonce: 1, last_user_nonce: 1 }
      ]
    }

## Transaction Manager

The roles of the Transaction Manager are:

- sign and send transactions to the main chain when requested
- make sure the current transactions make through, by possibly increasing the fees until a pre-established limit.
- working to meet deadlines for the submission of each transaction and
- answer if a certain transaction is being currently handled.

#### State

This contract doesn't need to store any data permanently and all its state can be obtained again by the Dispatcher Loop.

Therefore, although the Transaction Manager keeps track of the transactions that are currently being handled, this is not critical.

#### Calls

user nonce is read once and glued in the transaction until further notice.

The first type of call to the Transaction Manager is to request a new transaction to be sent:

    { origin: 0x6ac7ea33f8831ea9dcc53393aaa88b25a785dbf0,
      destination: 0xf778b86fa74e846c4f0a1fbd1335fe81c00a0c91,
      value: 9840000000000000,
      instance: 28,
      nonce: 23,
      deadline: 1574178405000,
      max_price: 234000000000000000,
      payload: "0xc6888fa10000000000000000000000",
    }

The return of this message is simply a OK/ERROR status.

After receiving such a message, the Transaction Manager will keep trying to send the above transaction: within the specified deadline, trying to minimize the cost and not going over the value `max_price`.
While the current process is trying to push this transaction, it is marked as "being handled" for the purpose of the request described below.

Another query that can be made to the Transaction Manager sees if some instance/nonce is being handled or not:

    { origin: 0x6ac7ea33f8831ea9dcc53393aaa88b25a785dbf0,
      destination: 0xf778b86fa74e846c4f0a1fbd1335fe81c00a0c91,
      instance: 28,
      nonce: 23
    }

And it returns true/false.

Should the Transaction Manager shut down, it forgets everything that is being handled and has to be put to work again by the Dispatcher.

## DApp Callback

This callback holds all the off-chain logic of the smart contract.
It simply uses gRPC to create a server for the Dispatcher.
This service will respond to any situation within the blockchain appropriately.

## State

This callback is completely stateless (to easy the developer's work).

## Calls

There is a single call that the DApp Callback should react to.
The input should look like this:

    { contract: 0xf778b86fa74e846c4f0a1fbd1335fe81c00a0c91,
      address: 0x6ac7ea33f8831ea9dcc53393aaa88b25a785dbf0,
      instance: 28,
      nonce: 23,
      state: "0xc6888fa1000000000000000000000000000"
    }

Note that `state` is the encoded form of something like: `[state: 2, queryArray: [0, 200, 400, 600]]`.

After receiving this call, the DApp Callback can use the network and interact with the File Manager until it is ready to submit a response in the form of a transaction.

The return of this call is a transaction exactly as described for the Transaction Manager.

## File Manager

The first implementation of the File Manager will act very much like a key-value table.
It receives a file, returning its Merkle-Tree Hash and stores the file for future retrieval.


