---
title: Priority
sidebarTitle: Priority
---

# Decision to choose a priority strategy

**status**: draft

## Context

Some transactions have higher priority for processing than others, so a weighting concept is used to sort the transactions accordingly. Therefore, the weight is defined in configurable labels as queues that the user assigns when submitting a transaction. Finally, transactions are fetched based on their weight, meaning the fetch operation will return a number of transactions proportional to the label weights. For example, if *label1* has a weight of 2 and *label2* has a weight of 1, the fetch operation will return 2 transactions from *label1* and 1 transaction from *label2*.

**Options:**
  - **SQL Query**: ?
  - **Memory**: ?

## Decision

?

**Rationale**
  - ?

