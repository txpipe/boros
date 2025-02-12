---
title: Priority
sidebarTitle: Priority
---

# Decision to choose a priority strategy

**status**: draft

## Context

Some transactions have higher priority for processing than others, so a weighting concept is used to sort the transactions accordingly. Therefore, the weight is defined in configurable labels as queues that the user assigns when submitting a transaction. Finally, transactions are fetched based on their weight, meaning the fetch operation will return a number of transactions proportional to the label weights. For example, if *label1* has a weight of 2 and *label2* has a weight of 1, the fetch operation will return 2 transactions from *label1* and 1 transaction from *label2*.

**Options:**
  - **SQL Query 1**: A query sorting based on delta((now - created_at) / 20s, avg to produce block), weight and datetime, so transactions with less weight but older will have more priority than new transactions with high weight because the delta increases the weight using the time.
  - **SQL Query 2**: Execute an initial query to fetch the state of the database, which is a report of data grouped by label/weight. Then, based on this state, fetch the next transaction from the selected group. The next query sorts the transactions by their creation date.
  - **Queue**: Execute a query to fetch pending transactions based on the labels/queues configured in the config file. A static quota of transactions defines the limit that can be sent to the fanout stage, and transactions are selected based on labels/queues, each having a weight.
  - **Memory**: Load transactions that should be submitted in memory and implement a rust pool iterator to fetch the next transaction based on label/weight. 

## Decision

We chose **Queue** because it allows the user to define a configurable queue, and it is the fairest method to sort transactions.

**Rationale**
  - Supports configurable queues.
  - Quote of transactions.
  - Handles when a transaction can be moved to the fanout stage.

