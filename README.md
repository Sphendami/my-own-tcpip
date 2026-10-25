# A Simple Network Protocol Stack Implementation for Learning

To deepen my understanding of network protocols, I implemented several of them from scratch.

This project implements:
- Ethernet frame transmission and reception, including minimum-size padding, CRC validation, MAC address learning, and frame forwarding.
- ARP address resolution, including an address cache and a queue for packets awaiting an ARP reply.
- IPv4 packet transmission, reception, and forwarding between subnets using direct and next-hop routes, with TTL decrementing.
- A simulated network of nodes, ports, and links that demonstrates end-to-end packet delivery through an Ethernet switch and an IP router.

After implementing the IP layer, I realized that I was more interested in how lower layers enable end-to-end communication than in how upper-layer protocols work. For that reason, despite "TCP" in the project name, this project does not implement a transport-layer protocol.