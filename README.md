# infrastructure
This provides the infrastructure to support the development of dApps, improving transaction and event management

![Image](https://i.postimg.cc/pdNK0whf/image.png)

So, the purpose of each of these components is described briefly. Later we will give an overview of the Dispatcher Loop, than a detailed description of each of the interfaces between these components.

### Dispatcher

This is a component that we offer to loop while observing the state of the blockchain and acting on the behalf of the dApp.
This component is very central in the sense that it communicates to all others in the system.
This apparent complexity is mitigated by the fact that it contains no state and can always gracefully recover from a power-down.

### Tx Manager

Whenever the dApp needs to send a transaction to the blockchain, it has the possibility to do it through our Tx Manager, which will take care of all the bureaucracy of dealing with Ethereum and also the async nature of transactions.

### State Manager

DApps written for Cartesi will work best if their smart contracts have some "getter functions" that are predefined by us, guiding somehow the inner workings of the contract.
Since these getter functions are standard, we have a component dedicated to read these data, which abstracts away:
 - the specific blockchain that we are dealing with,
 - whether we work with a full or light node

### DApp Callback

This is where the dApp-specific action takes place.
In order to make a Cartesi dApp, a developer has to implement three things: a few smart contracts, some machines to run in our emulator and the "DApp Callback".

This module is called by the Dispatcher, with all the state information that it needs in order to make decisions, then it can access files on the File Manager and return some action to the Dispatcher in the form of transactions.

### Configuration File

This is a simple file holding data that is specific of the user's installation of the dApp, like the user's address.

### Ethereum Node

This is self explanatory. Crazy complex, but self explanatory.

## The Dispatcher Loop

The Dispatcher should be stateless, so that in the event of a power-down, it can recover seamlessly.
Imagining that the Dispatcher just woke up, it will perform the following steps in order:

1. Open the Configuration File to collect data which is specific to this user. Examples: his/her address and contracts that should be watched.
1. Contact the State Manager to obtain all the blockchain information that is relevant to the user. Examples: in contract partition, instance 17, the state is WaitingQuery.
1. All this information is passed to the DApp Callback, which will have to take a decision on how to proceed (more details later). The DApp Callback returns to the Dispatcher one (or more) transactions that should be sent to the blockchain.
1. The Dispatcher sends these transactions to the Tx Manager that will make sure they are inserted into the blockchain.
