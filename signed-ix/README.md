A basic program which shows how to verify that a top level instruction has been signed by a known authority in another instruction

Use cases:

Executing in a permissionless manner an instruction that is correct according to an off-chain authority

Jupiter swaps cannot be constructed on-chain due to obvious compute costs and the wide read lock on all AMM related accounts necessary to quote.
For flexibility we don't want to sign a given tx, the act of swapping is self contained and the integrators relying on it are responsible for all subsequent validations. Integrators might also not desire broadcasting the full instruction to this service.

So we would introduce a pull based swap, where we sign a top level ix for a signer, which would guarantee the swap is the best possible for an interval of time.
