mod node;

use node::*;

fn main() {
    // Create nodes
    let mut nodes = (0..2).map(|id| Node::new(id)).collect::<Vec<_>>();

    // Connect nodes
    let [node0, node1] = nodes.get_disjoint_mut([0, 1]).unwrap();
    Node::connect(node0, node1);

    // Get meta port for interaction from main thread
    let meta_port_node0 = nodes[0].meta_port();

    // Run nodes
    let handles = nodes.into_iter().map(Node::run).collect::<Vec<_>>();

    // Send initial message
    meta_port_node0.send(vec![0, 1, 2, 3]);

    // Wait for nodes to finish
    for handle in handles {
        handle.join().unwrap();
    }
}
